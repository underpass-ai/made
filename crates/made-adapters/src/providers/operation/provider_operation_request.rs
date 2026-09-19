use super::{OperationCorrelation, ProviderIdentity};

/// A request handed to a provider operation adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderOperationRequest {
    identity: ProviderIdentity,
    correlation: OperationCorrelation,
}

impl ProviderOperationRequest {
    #[must_use]
    pub const fn new(identity: ProviderIdentity, correlation: OperationCorrelation) -> Self {
        Self {
            identity,
            correlation,
        }
    }
    #[must_use]
    pub fn identity(&self) -> &ProviderIdentity {
        &self.identity
    }
    #[must_use]
    pub fn correlation(&self) -> &OperationCorrelation {
        &self.correlation
    }
}
