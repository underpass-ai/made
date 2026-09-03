//! In-memory [`ContractRegistryPort`] backed by a `RwLock<BTreeMap>`.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::ContractRegistryPort;
use made_core::value_objects::{OutputContract, OutputContractId};
use tokio::sync::RwLock;

/// In-memory contract registry keyed by `contract_id`.
///
/// Cheap to `Clone`; internal state is shared through `Arc<RwLock>`.
#[derive(Debug, Default, Clone)]
pub struct InMemoryContractRegistry {
    inner: Arc<RwLock<BTreeMap<OutputContractId, OutputContract>>>,
}

impl InMemoryContractRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of contracts currently registered. Read-only helper for
    /// diagnostics and tests.
    pub async fn len(&self) -> usize {
        self.inner.read().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.inner.read().await.is_empty()
    }
}

#[async_trait]
impl ContractRegistryPort for InMemoryContractRegistry {
    async fn register(&self, contract: OutputContract) -> Result<(), DomainError> {
        let mut map = self.inner.write().await;
        if map.contains_key(contract.contract_id()) {
            return Err(DomainError::AlreadyExists { what: "contract" });
        }
        map.insert(contract.contract_id().to_owned(), contract);
        Ok(())
    }

    async fn get(&self, contract_id: &OutputContractId) -> Result<OutputContract, DomainError> {
        self.inner
            .read()
            .await
            .get(contract_id)
            .cloned()
            .ok_or(DomainError::NotFound { what: "contract" })
    }

    async fn list(&self) -> Result<Vec<OutputContract>, DomainError> {
        Ok(self.inner.read().await.values().cloned().collect())
    }

    async fn delete(&self, contract_id: &OutputContractId) -> Result<(), DomainError> {
        self.inner
            .write()
            .await
            .remove(contract_id)
            .map(|_| ())
            .ok_or(DomainError::NotFound { what: "contract" })
    }

    async fn contains(&self, contract_id: &OutputContractId) -> Result<bool, DomainError> {
        Ok(self.inner.read().await.contains_key(contract_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use made_core::value_objects::OutputFormat;
    use std::collections::BTreeMap;

    fn contract(id: &str) -> OutputContract {
        OutputContract::new(id, OutputFormat::JsonObject, BTreeMap::new()).unwrap()
    }

    fn id(value: &str) -> OutputContractId {
        OutputContractId::new(value).unwrap()
    }

    #[tokio::test]
    async fn register_then_get_roundtrips() {
        let reg = InMemoryContractRegistry::new();
        reg.register(contract("triage-v1")).await.unwrap();
        let got = reg.get(&id("triage-v1")).await.unwrap();
        assert_eq!(got.contract_id(), "triage-v1");
    }

    #[tokio::test]
    async fn duplicate_register_rejected() {
        let reg = InMemoryContractRegistry::new();
        reg.register(contract("x")).await.unwrap();
        let err = reg.register(contract("x")).await.unwrap_err();
        assert!(matches!(
            err,
            DomainError::AlreadyExists { what: "contract" }
        ));
    }

    #[tokio::test]
    async fn missing_get_is_not_found() {
        let reg = InMemoryContractRegistry::new();
        let err = reg.get(&id("nope")).await.unwrap_err();
        assert!(matches!(err, DomainError::NotFound { what: "contract" }));
    }

    #[tokio::test]
    async fn list_reports_everything() {
        let reg = InMemoryContractRegistry::new();
        reg.register(contract("a")).await.unwrap();
        reg.register(contract("b")).await.unwrap();
        let all = reg.list().await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn delete_removes_entry_and_then_404s() {
        let reg = InMemoryContractRegistry::new();
        reg.register(contract("x")).await.unwrap();
        reg.delete(&id("x")).await.unwrap();
        assert!(reg.is_empty().await);

        let err = reg.delete(&id("x")).await.unwrap_err();
        assert!(matches!(err, DomainError::NotFound { what: "contract" }));
    }

    #[tokio::test]
    async fn contains_reflects_state() {
        let reg = InMemoryContractRegistry::new();
        assert!(!reg.contains(&id("x")).await.unwrap());
        reg.register(contract("x")).await.unwrap();
        assert!(reg.contains(&id("x")).await.unwrap());
    }

    #[tokio::test]
    async fn clone_shares_state() {
        let a = InMemoryContractRegistry::new();
        let b = a.clone();
        a.register(contract("x")).await.unwrap();
        assert_eq!(b.len().await, 1);
    }
}
