use super::{ProviderContractError, ProviderModel, ProviderName, ProviderOperationId};

/// The non-secret identity that must survive a provider operation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderIdentity {
    operation_id: ProviderOperationId,
    provider: ProviderName,
    model: ProviderModel,
}

impl ProviderIdentity {
    /// Build an identity from the operation, provider and model labels.
    pub fn new(
        operation_id: impl Into<String>,
        provider: impl Into<String>,
        model: impl Into<String>,
    ) -> Result<Self, ProviderContractError> {
        Ok(Self {
            operation_id: ProviderOperationId::new(operation_id)?,
            provider: ProviderName::new(provider)?,
            model: ProviderModel::new(model)?,
        })
    }

    #[must_use]
    pub fn operation_id(&self) -> &ProviderOperationId {
        &self.operation_id
    }
    #[must_use]
    pub fn provider(&self) -> &ProviderName {
        &self.provider
    }
    #[must_use]
    pub fn model(&self) -> &ProviderModel {
        &self.model
    }
}
