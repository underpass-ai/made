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
use crate::value_objects::OutputContract;

#[async_trait]
pub trait ContractRegistryPort: Send + Sync {
    /// Store a freshly defined contract. Fails with
    /// [`DomainError::AlreadyExists`] if a contract for the same
    /// `contract_id` already exists.
    async fn register(&self, contract: OutputContract) -> Result<(), DomainError>;

    /// Fetch the contract for an id. Returns
    /// [`DomainError::NotFound`] when absent.
    async fn get(&self, contract_id: &str) -> Result<OutputContract, DomainError>;

    /// Enumerate every registered contract. Ordering is unspecified.
    async fn list(&self) -> Result<Vec<OutputContract>, DomainError>;

    /// Remove the contract for an id. Returns
    /// [`DomainError::NotFound`] when absent.
    async fn delete(&self, contract_id: &str) -> Result<(), DomainError>;

    /// Cheap existence check that avoids materialising the contract.
    async fn contains(&self, contract_id: &str) -> Result<bool, DomainError>;
}
