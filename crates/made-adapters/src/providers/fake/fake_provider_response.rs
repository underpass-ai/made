use super::super::operation::{FinishReason, Observed, ProviderErrorClassification, Usage};
use made_core::value_objects::DurationMs;

/// A fixed response returned by [`super::FakeProviderAdapter`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FakeProviderResponse {
    Success {
        latency: DurationMs,
        usage: Observed<Usage>,
        finish: FinishReason,
    },
    Error {
        latency: DurationMs,
        usage: Observed<Usage>,
        error: ProviderErrorClassification,
    },
}

impl FakeProviderResponse {
    #[must_use]
    pub const fn success(latency_ms: u64, usage: Usage, finish: FinishReason) -> Self {
        Self::Success {
            latency: DurationMs::from_millis(latency_ms),
            usage: Observed::declared_by_fixture(usage),
            finish,
        }
    }
    #[must_use]
    pub const fn error(latency_ms: u64, usage: Usage, error: ProviderErrorClassification) -> Self {
        Self::Error {
            latency: DurationMs::from_millis(latency_ms),
            usage: Observed::declared_by_fixture(usage),
            error,
        }
    }
}
