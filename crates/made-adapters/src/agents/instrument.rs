//! Shared instrumentation for HTTP provider-agent calls.
//!
//! [`ProviderCallGuard`] wraps a single completion call so its latency
//! and in-flight accounting are recorded on **every** return path —
//! including the `?` early-returns of the four error sites — without a
//! hand-written `finally`. Increment-on-enter / record-and-decrement-on-
//! drop keeps the in-flight gauge balanced even when a call fails.

use std::time::Instant;

use made_core::ports::MetricsRecorderPort;
use made_core::value_objects::{DurationMs, LlmErrorKind, TokenUsage};

pub(super) struct ProviderCallGuard<'a> {
    metrics: &'a dyn MetricsRecorderPort,
    provider: &'static str,
    operation: &'a str,
    started: Instant,
}

impl<'a> ProviderCallGuard<'a> {
    /// Mark a call as started: increment the provider's in-flight gauge
    /// and begin timing. Hold the returned guard for the call's duration.
    pub(super) fn enter(
        metrics: &'a dyn MetricsRecorderPort,
        provider: &'static str,
        operation: &'a str,
    ) -> Self {
        metrics.inc_provider_in_flight(provider);
        Self {
            metrics,
            provider,
            operation,
            started: Instant::now(),
        }
    }

    pub(super) fn record_error(&self, kind: LlmErrorKind) {
        self.metrics.record_provider_error(self.provider, kind);
        record_error(kind);
    }

    pub(super) fn record_tokens(&self, usage: TokenUsage) {
        self.metrics.record_provider_tokens(self.provider, usage);
        record_tokens(usage);
    }
}

pub(super) fn record_success() {
    tracing::Span::current().record("outcome", "success");
}

pub(super) fn record_error(kind: LlmErrorKind) {
    let span = tracing::Span::current();
    span.record("outcome", "error");
    span.record("error_kind", kind.as_label());
}

pub(super) fn record_tokens(usage: TokenUsage) {
    #[cfg(not(feature = "otel"))]
    {
        let span = tracing::Span::current();
        span.record("prompt_tokens", u64::from(usage.prompt()));
        span.record("completion_tokens", u64::from(usage.completion()));
    }
    #[cfg(feature = "otel")]
    {
        use opentelemetry::trace::TraceContextExt as _;
        use tracing_opentelemetry::OpenTelemetrySpanExt as _;

        let context = tracing::Span::current().context();
        let span = context.span();
        span.set_attribute(opentelemetry::KeyValue::new(
            "prompt_tokens",
            i64::from(usage.prompt()),
        ));
        span.set_attribute(opentelemetry::KeyValue::new(
            "completion_tokens",
            i64::from(usage.completion()),
        ));
    }
}

impl Drop for ProviderCallGuard<'_> {
    fn drop(&mut self) {
        let elapsed = DurationMs::from_millis(self.started.elapsed().as_millis() as u64);
        self.metrics
            .observe_provider_request(self.provider, self.operation, elapsed);
        self.metrics.dec_provider_in_flight(self.provider);
    }
}
