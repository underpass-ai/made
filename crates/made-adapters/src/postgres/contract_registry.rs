use super::council_journal_store::{append, begin};
use super::error::{serde_to_domain, sqlx_to_domain};
use super::PostgresPool;
use async_trait::async_trait;
use made_core::entities::CouncilJournalEvent;
use made_core::error::DomainError;
use made_core::ports::ContractRegistryPort;
use made_core::value_objects::{AuthorizationEvidence, OutputContract, OutputContractId};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct PostgresContractRegistry {
    pool: PostgresPool,
}
impl PostgresContractRegistry {
    #[must_use]
    pub fn new(pool: PostgresPool) -> Self {
        Self { pool }
    }
}
#[async_trait]
impl ContractRegistryPort for PostgresContractRegistry {
    async fn register(&self, contract: OutputContract) -> Result<(), DomainError> {
        self.register_authorized(contract, None).await
    }
    async fn register_authorized(
        &self,
        contract: OutputContract,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<(), DomainError> {
        let mut tx = begin(&self.pool).await?;
        let body =
            serde_json::to_value(&contract).map_err(|e| serde_to_domain(&e, "encode contract"))?;
        let result = sqlx::query("INSERT INTO council_contracts (contract_id, body) VALUES ($1, $2) ON CONFLICT (contract_id) DO NOTHING")
            .bind(contract.contract_id().as_str()).bind(body).execute(&mut *tx).await
            .map_err(|e| sqlx_to_domain(e, "register contract"))?;
        if result.rows_affected() == 0 {
            return Err(DomainError::AlreadyExists { what: "contract" });
        }
        append(
            &mut tx,
            CouncilJournalEvent::ContractRegistered(contract),
            authorization,
        )
        .await?;
        tx.commit()
            .await
            .map_err(|e| sqlx_to_domain(e, "commit contract"))
    }
    async fn get(&self, id: &OutputContractId) -> Result<OutputContract, DomainError> {
        let body: Value =
            sqlx::query_scalar("SELECT body FROM council_contracts WHERE contract_id = $1")
                .bind(id.as_str())
                .fetch_optional(self.pool.inner())
                .await
                .map_err(|e| sqlx_to_domain(e, "get contract"))?
                .ok_or(DomainError::NotFound { what: "contract" })?;
        serde_json::from_value(body).map_err(|e| serde_to_domain(&e, "decode contract"))
    }
    async fn list(&self) -> Result<Vec<OutputContract>, DomainError> {
        let bodies: Vec<Value> =
            sqlx::query_scalar("SELECT body FROM council_contracts ORDER BY contract_id")
                .fetch_all(self.pool.inner())
                .await
                .map_err(|e| sqlx_to_domain(e, "list contracts"))?;
        bodies
            .into_iter()
            .map(|body| {
                serde_json::from_value(body).map_err(|e| serde_to_domain(&e, "decode contract"))
            })
            .collect()
    }
    async fn delete(&self, id: &OutputContractId) -> Result<(), DomainError> {
        self.delete_authorized(id, None).await
    }
    async fn delete_authorized(
        &self,
        id: &OutputContractId,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<(), DomainError> {
        let mut tx = begin(&self.pool).await?;
        let result = sqlx::query("DELETE FROM council_contracts WHERE contract_id = $1")
            .bind(id.as_str())
            .execute(&mut *tx)
            .await
            .map_err(|e| sqlx_to_domain(e, "delete contract"))?;
        if result.rows_affected() == 0 {
            return Err(DomainError::NotFound { what: "contract" });
        }
        append(
            &mut tx,
            CouncilJournalEvent::ContractDeleted(id.clone()),
            authorization,
        )
        .await?;
        tx.commit()
            .await
            .map_err(|e| sqlx_to_domain(e, "commit contract deletion"))
    }
    async fn contains(&self, id: &OutputContractId) -> Result<bool, DomainError> {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM council_contracts WHERE contract_id = $1)")
            .bind(id.as_str())
            .fetch_one(self.pool.inner())
            .await
            .map_err(|e| sqlx_to_domain(e, "contains contract"))
    }
}
