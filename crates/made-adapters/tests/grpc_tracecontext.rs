//! Integration test: `link_span_to_metadata` in `grpc::tracecontext`
//! threads an incoming W3C `traceparent` into the span MADE exports.
//!
//! The assertion is on exported span data, not on an in-process getter:
//! what matters is the parentage a collector receives. `tracing`'s own
//! view of a span's OTel context is an implementation detail that has
//! changed shape across bridge versions, and testing it once hid a real
//! question behind a green light.
//!
//! Owns its own integration-test binary so the
//! `tracing-opentelemetry` bridge + global propagator install
//! don't race with unit tests' thread-local subscribers.

#![cfg(feature = "otel")]

use std::sync::OnceLock;

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider, SimpleSpanProcessor};
use tonic::metadata::MetadataValue;
use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _};

const REMOTE_TRACE_ID: &str = "0af7651916cd43dd8448eb211c80319c";
const REMOTE_SPAN_ID: &str = "b7ad6b7169203331";

/// One bridge per test binary, exporting into memory so a test can read
/// back exactly what would have been shipped over OTLP.
fn install_bridge() -> InMemorySpanExporter {
    static EXPORTER: OnceLock<InMemorySpanExporter> = OnceLock::new();
    EXPORTER
        .get_or_init(|| {
            opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());
            let exporter = InMemorySpanExporter::default();
            let provider = SdkTracerProvider::builder()
                .with_span_processor(SimpleSpanProcessor::new(exporter.clone()))
                .build();
            let tracer = provider.tracer("test");
            opentelemetry::global::set_tracer_provider(provider);
            tracing_subscriber::registry()
                .with(
                    tracing_opentelemetry::layer()
                        .with_tracer(tracer)
                        // Mirrors the production subscriber; see
                        // `made::telemetry`. Without this, entering a span
                        // freezes its parentage before a handler can adopt
                        // the caller's.
                        .with_context_activation(false),
                )
                .init();
            exporter
        })
        .clone()
}

fn exported(exporter: &InMemorySpanExporter, name: &str) -> opentelemetry_sdk::trace::SpanData {
    exporter
        .get_finished_spans()
        .expect("finished spans must be readable")
        .into_iter()
        .find(|span| span.name == name)
        .unwrap_or_else(|| panic!("no exported span named `{name}`"))
}

#[tokio::test]
async fn link_span_to_metadata_adopts_incoming_traceparent_as_parent() {
    let exporter = install_bridge();

    let mut request = tonic::Request::new(());
    request.metadata_mut().insert(
        "traceparent",
        MetadataValue::from_static("00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01"),
    );

    {
        let span = tracing::info_span!("rpc.test.link");
        let _enter = span.enter();
        made_adapters::__test_only::link_span_to_metadata(&request);
    }

    let span = exported(&exporter, "rpc.test.link");
    assert_eq!(
        format!("{:032x}", span.span_context.trace_id()),
        REMOTE_TRACE_ID,
        "the exported span must belong to the caller's trace"
    );
    assert_eq!(
        format!("{:016x}", span.parent_span_id),
        REMOTE_SPAN_ID,
        "the exported span must hang off the caller's span"
    );
}

#[tokio::test]
async fn a_request_without_traceparent_starts_its_own_trace() {
    let exporter = install_bridge();

    let request = tonic::Request::new(());

    {
        let span = tracing::info_span!("rpc.test.root");
        let _enter = span.enter();
        made_adapters::__test_only::link_span_to_metadata(&request);
    }

    let span = exported(&exporter, "rpc.test.root");
    assert_ne!(
        format!("{:032x}", span.span_context.trace_id()),
        REMOTE_TRACE_ID,
        "a missing traceparent must not pick up a remote trace id"
    );
    assert!(
        !span.parent_span_id.to_string().is_empty(),
        "the span must still export a parent id field"
    );
    assert_eq!(
        format!("{:016x}", span.parent_span_id),
        "0000000000000000",
        "self-originated work is a root span"
    );
}
