use super::{ProviderContractError, ProviderOperationObservation, ProviderOperationRequest};
use std::fmt;

/// An adapter that observes one provider operation without owning authority over it.
pub trait ProviderOperationAdapter: fmt::Debug + Send + Sync {
    fn execute(
        &self,
        request: &ProviderOperationRequest,
    ) -> Result<ProviderOperationObservation, ProviderContractError>;
}
