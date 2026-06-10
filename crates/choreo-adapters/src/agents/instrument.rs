//! Shared instrumentation for HTTP provider-agent calls.
//!
//! [`ProviderCallGuard`] wraps a single completion call so its latency
//! and in-flight accounting are recorded on **every** return path —
//! including the `?` early-returns of the four error sites — without a
//! hand-written `finally`. Increment-on-enter / record-and-decrement-on-
//! drop keeps the in-flight gauge balanced even when a call fails.

use std::time::Instant;

use choreo_core::ports::MetricsRecorderPort;
use choreo_core::value_objects::DurationMs;

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
}

impl Drop for ProviderCallGuard<'_> {
    fn drop(&mut self) {
        let elapsed = DurationMs::from_millis(self.started.elapsed().as_millis() as u64);
        self.metrics
            .observe_provider_request(self.provider, self.operation, elapsed);
        self.metrics.dec_provider_in_flight(self.provider);
    }
}
