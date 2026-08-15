//! Telemetry pipeline setup.
//!
//! Two build-time variants, selected by the `otel` Cargo feature:
//!
//! - **Without `otel`** (default): a plain JSON-format subscriber
//!   goes to stdout. Spans are observable in-process only.
//!
//! - **With `otel`** + `MADE_OTLP_ENDPOINT` set: the same JSON
//!   subscriber is layered together with a `tracing-opentelemetry`
//!   bridge that exports spans over OTLP (gRPC transport) to the
//!   configured collector. Spans acquire real OTel trace/span IDs
//!   and cross-service correlation becomes available.
//!
//! - **With `otel`** but `MADE_OTLP_ENDPOINT` unset: the binary
//!   behaves exactly as without the feature — JSON only. No silent
//!   background exporter, no wasted connections.
//!
//! The returned [`TelemetryGuard`] owns the tracer provider's
//! lifetime so `main` can drop it at shutdown and flush any
//! buffered spans.

use anyhow::Result;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// RAII handle returned by [`init_tracing`]. Drop on shutdown so the
/// OTel exporter flushes buffered spans before the process exits.
#[must_use]
pub struct TelemetryGuard {
    #[cfg(feature = "otel")]
    provider: Option<opentelemetry_sdk::trace::SdkTracerProvider>,
}

impl TelemetryGuard {
    #[cfg(not(feature = "otel"))]
    fn noop() -> Self {
        Self {}
    }

    #[cfg(feature = "otel")]
    fn noop() -> Self {
        Self { provider: None }
    }
}

impl std::fmt::Debug for TelemetryGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TelemetryGuard").finish()
    }
}

#[cfg(feature = "otel")]
impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Some(provider) = self.provider.take() {
            // `shutdown` drains in-flight spans and closes the
            // exporter. Any error here is purely at-shutdown and
            // gets logged via the global tracer hook.
            let _ = provider.shutdown();
        }
    }
}

fn env_filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
}

/// Build the mutual-TLS client config for the OTLP exporter from
/// `MADE_OTLP_TLS_{CA,CERT,KEY}_PATH`, with an optional
/// `MADE_OTLP_TLS_DOMAIN_NAME` SNI override for when the collector's
/// Service name differs from its certificate SAN. Returns `None` when the
/// TLS env is unset, leaving the export plaintext.
#[cfg(feature = "otel")]
/// The OTLP exporter links its own tonic, one major ahead of the one the
/// gRPC server uses. Its TLS config is therefore a different type with the
/// same name, and it must be built from the exporter's re-export or it will
/// not typecheck — a compiler error worth keeping rather than papering over
/// with a workspace-wide tonic bump the server does not need.
fn otlp_client_tls_from_env(
) -> Result<Option<opentelemetry_otlp::tonic_types::transport::ClientTlsConfig>> {
    use anyhow::Context as _;
    use opentelemetry_otlp::tonic_types::transport::{Certificate, ClientTlsConfig, Identity};

    let read = |var: &str| {
        std::env::var(var)
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    };
    let (Some(ca), Some(cert), Some(key)) = (
        read("MADE_OTLP_TLS_CA_PATH"),
        read("MADE_OTLP_TLS_CERT_PATH"),
        read("MADE_OTLP_TLS_KEY_PATH"),
    ) else {
        return Ok(None);
    };
    let ca_pem = std::fs::read(&ca).with_context(|| format!("reading OTLP CA {ca}"))?;
    let cert_pem = std::fs::read(&cert).with_context(|| format!("reading OTLP cert {cert}"))?;
    let key_pem = std::fs::read(&key).with_context(|| format!("reading OTLP key {key}"))?;
    let mut tls = ClientTlsConfig::new()
        .ca_certificate(Certificate::from_pem(ca_pem))
        .identity(Identity::from_pem(cert_pem, key_pem));
    if let Some(domain) = read("MADE_OTLP_TLS_DOMAIN_NAME") {
        tls = tls.domain_name(domain);
    }
    Ok(Some(tls))
}

#[cfg(not(feature = "otel"))]
pub fn init_tracing() -> Result<TelemetryGuard> {
    tracing_subscriber::registry()
        .with(env_filter())
        .with(fmt::layer().json())
        .init();
    Ok(TelemetryGuard::noop())
}

#[cfg(feature = "otel")]
pub fn init_tracing() -> Result<TelemetryGuard> {
    use opentelemetry::global;
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry_otlp::{WithExportConfig as _, WithTonicConfig as _};
    use opentelemetry_sdk::{
        propagation::TraceContextPropagator, trace::SdkTracerProvider, Resource,
    };
    use opentelemetry_semantic_conventions as sc;

    // Every binary build registers the W3C propagator so any
    // interceptor (gRPC, NATS) can extract/inject `traceparent`
    // even when no exporter is configured. Propagation is cheap.
    global::set_text_map_propagator(TraceContextPropagator::new());

    let filter = env_filter();
    let fmt_layer = fmt::layer().json();

    let Some(endpoint) = std::env::var("MADE_OTLP_ENDPOINT")
        .ok()
        .filter(|s| !s.trim().is_empty())
    else {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt_layer)
            .init();
        return Ok(TelemetryGuard::noop());
    };

    let mut exporter_builder = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint.clone());
    // mTLS to the collector when configured. The Underpass planes speak
    // mutual TLS end-to-end, so the OTLP export presents the same client
    // identity the runtime executor uses; the collector verifies it against
    // its `client_ca_file`. Stays plaintext when the TLS env is unset.
    let mtls = if let Some(tls) = otlp_client_tls_from_env()? {
        exporter_builder = exporter_builder.with_tls_config(tls);
        true
    } else {
        false
    };
    let exporter = exporter_builder.build()?;

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(
            Resource::builder()
                .with_attributes([
                    opentelemetry::KeyValue::new(sc::resource::SERVICE_NAME, "made"),
                    opentelemetry::KeyValue::new(
                        sc::resource::SERVICE_VERSION,
                        env!("CARGO_PKG_VERSION"),
                    ),
                ])
                .build(),
        )
        .build();

    global::set_tracer_provider(provider.clone());
    // Context activation is off on purpose. With it on, entering a span
    // starts its OpenTelemetry context immediately, and a started span can
    // no longer be re-parented — which is exactly what every RPC handler
    // does when it adopts an incoming `traceparent` (see
    // `made_adapters::grpc::tracecontext`). Nothing here injects context
    // into outbound calls, so the only thing activation would buy us is the
    // loss of caller parentage.
    let otel_layer = tracing_opentelemetry::layer()
        .with_tracer(provider.tracer("made"))
        .with_context_activation(false);

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .with(otel_layer)
        .init();

    // Logged after `.init()` so it actually reaches a subscriber.
    tracing::info!(endpoint, mtls, "otlp exporter wired");
    Ok(TelemetryGuard {
        provider: Some(provider),
    })
}
