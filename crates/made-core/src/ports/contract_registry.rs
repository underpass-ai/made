//! [`ContractRegistryPort`] — registry of [`OutputContract`]s keyed by
//! their `contract_id`.
//!
//! Mirrors [`CouncilRegistryPort`](super::CouncilRegistryPort) at the
//! verb level (`register` / `get` / `list` / `delete` / `contains`).
//! The deliberate omission is `replace`: contracts are immutable per
//! id in this slice. Updating a contract means deleting it and
//! registering a new one, which forces the caller (or operator) to
//! confront the impact on in-flight consumers explicitly.

use async_trait::async_trait;

use crate::error::DomainError;
use crate::value_objects::{AuthorizationEvidence, OutputContract, OutputContractId};

#[async_trait]
pub trait ContractRegistryPort: Send + Sync {
    /// Store a freshly defined contract. Fails with
    /// [`DomainError::AlreadyExists`] if a contract for the same
    /// `contract_id` already exists.
    async fn register(&self, contract: OutputContract) -> Result<(), DomainError>;
    async fn register_authorized(
        &self,
        contract: OutputContract,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<(), DomainError> {
        reject_unsupported(authorization.as_ref())?;
        self.register(contract).await
    }

    /// Fetch the contract for an id. Returns
    /// [`DomainError::NotFound`] when absent.
    async fn get(&self, contract_id: &OutputContractId) -> Result<OutputContract, DomainError>;

    /// Enumerate every registered contract. Ordering is unspecified.
    async fn list(&self) -> Result<Vec<OutputContract>, DomainError>;

    /// Remove the contract for an id. Returns
    /// [`DomainError::NotFound`] when absent.
    async fn delete(&self, contract_id: &OutputContractId) -> Result<(), DomainError>;
    async fn delete_authorized(
        &self,
        contract_id: &OutputContractId,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<(), DomainError> {
        reject_unsupported(authorization.as_ref())?;
        self.delete(contract_id).await
    }

    /// Cheap existence check that avoids materialising the contract.
    async fn contains(&self, contract_id: &OutputContractId) -> Result<bool, DomainError>;
}

fn reject_unsupported(authorization: Option<&AuthorizationEvidence>) -> Result<(), DomainError> {
    if authorization.is_some() {
        return Err(DomainError::InvariantViolated {
            reason: "contract registry adapter cannot persist authorization evidence",
        });
    }
    Ok(())
}
