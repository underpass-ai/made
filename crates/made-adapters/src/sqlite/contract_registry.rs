use super::SqliteCouncilStore;
use crate::engine::Table;
use async_trait::async_trait;
use made_core::entities::CouncilJournalEvent;
use made_core::error::DomainError;
use made_core::ports::ContractRegistryPort;
use made_core::value_objects::{OutputContract, OutputContractId};

#[derive(Debug, Clone)]
pub struct SqliteContractRegistry {
    store: SqliteCouncilStore,
}
impl SqliteContractRegistry {
    #[must_use]
    pub fn new(store: SqliteCouncilStore) -> Self {
        Self { store }
    }
}
#[async_trait]
impl ContractRegistryPort for SqliteContractRegistry {
    async fn register(&self, contract: OutputContract) -> Result<(), DomainError> {
        self.store
            .insert(
                Table::CouncilContracts,
                contract.contract_id().to_string(),
                contract.clone(),
                "contract",
                CouncilJournalEvent::ContractRegistered(contract),
            )
            .await
    }
    async fn get(&self, id: &OutputContractId) -> Result<OutputContract, DomainError> {
        self.store
            .get(Table::CouncilContracts, id.to_string(), "contract")
            .await
    }
    async fn list(&self) -> Result<Vec<OutputContract>, DomainError> {
        self.store.list(Table::CouncilContracts).await
    }
    async fn delete(&self, id: &OutputContractId) -> Result<(), DomainError> {
        self.store
            .delete(
                Table::CouncilContracts,
                id.to_string(),
                "contract",
                CouncilJournalEvent::ContractDeleted(id.clone()),
            )
            .await
    }
    async fn contains(&self, id: &OutputContractId) -> Result<bool, DomainError> {
        self.store
            .contains(Table::CouncilContracts, id.to_string())
            .await
    }
}
