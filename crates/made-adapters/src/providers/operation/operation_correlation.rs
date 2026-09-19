use super::{ProviderContractError, ProviderRequestId};

/// Correlation that ties a provider request back to the caller and its trace.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OperationCorrelation {
    request_id: ProviderRequestId,
    correlation_id: Option<ProviderRequestId>,
}

impl OperationCorrelation {
    pub fn new(request_id: impl Into<String>) -> Result<Self, ProviderContractError> {
        Ok(Self {
            request_id: ProviderRequestId::new(request_id)?,
            correlation_id: None,
        })
    }
    pub fn with_correlation_id(
        mut self,
        correlation_id: impl Into<String>,
    ) -> Result<Self, ProviderContractError> {
        self.correlation_id = Some(ProviderRequestId::new(correlation_id)?);
        Ok(self)
    }
    #[must_use]
    pub fn request_id(&self) -> &ProviderRequestId {
        &self.request_id
    }
    #[must_use]
    pub fn correlation_id(&self) -> Option<&ProviderRequestId> {
        self.correlation_id.as_ref()
    }
}
