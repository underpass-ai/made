use std::future::Future;

use made_core::value_objects::TraceContext;
use opentelemetry::trace::TraceContextExt as _;
use tracing_opentelemetry::OpenTelemetrySpanExt as _;

tokio::task_local! {
    static ACTIVE_CEREMONY_TRACE: TraceContext;
}

/// Keeps one trace context active across all appends made by an operation.
#[derive(Debug)]
pub struct CeremonyTraceScope;

impl CeremonyTraceScope {
    pub async fn run<F>(trace: TraceContext, future: F) -> F::Output
    where
        F: Future,
    {
        ACTIVE_CEREMONY_TRACE.scope(trace, future).await
    }
}

/// The operation trace, then the current OpenTelemetry span, then a new root.
pub(crate) fn current_trace_context() -> TraceContext {
    ACTIVE_CEREMONY_TRACE
        .try_with(Clone::clone)
        .ok()
        .or_else(trace_from_current_span)
        .unwrap_or_else(TraceContext::generate)
}

fn trace_from_current_span() -> Option<TraceContext> {
    let context = tracing::Span::current().context();
    let span = context.span();
    let span_context = span.span_context();
    if !span_context.is_valid() {
        return None;
    }
    TraceContext::new(
        span_context.trace_id().to_string(),
        span_context.span_id().to_string(),
        format!("{:02x}", span_context.trace_flags().to_u8()),
    )
    .ok()
}
