use super::{
    FinishClassification, Observed, OperationCorrelation, ProviderIdentity,
    ProviderOperationRequest, Usage,
};
use made_core::value_objects::DurationMs;

/// Result of observing one provider operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderOperationObservation {
    request: ProviderOperationRequest,
    latency: Observed<DurationMs>,
    usage: Observed<Usage>,
    finish: Observed<FinishClassification>,
}

impl ProviderOperationObservation {
    #[must_use]
    pub const fn new(
        request: ProviderOperationRequest,
        latency: Observed<DurationMs>,
        usage: Observed<Usage>,
        finish: Observed<FinishClassification>,
    ) -> Self {
        Self {
            request,
            latency,
            usage,
            finish,
        }
    }
    #[must_use]
    pub fn identity(&self) -> &ProviderIdentity {
        self.request.identity()
    }
    #[must_use]
    pub fn correlation(&self) -> &OperationCorrelation {
        self.request.correlation()
    }
    #[must_use]
    pub fn latency(&self) -> &Observed<DurationMs> {
        &self.latency
    }
    #[must_use]
    pub fn usage(&self) -> &Observed<Usage> {
        &self.usage
    }
    #[must_use]
    pub fn finish(&self) -> &Observed<FinishClassification> {
        &self.finish
    }
}
