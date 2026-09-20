//! Runs of an agentic system on Postgres.

use async_trait::async_trait;
use made_core::entities::AgenticSystemExecution;
use made_core::error::DomainError;
use made_core::ports::{
    AgenticSystemExecutionCreation, AgenticSystemExecutionStorePort, AgenticSystemExecutionUpdate,
};
use made_core::value_objects::{AgenticSystemExecutionId, AgenticSystemId};
use sqlx::{PgPool, Row};
use time::OffsetDateTime;

use super::ceremony_store::{decode, encode, sqlx_error};
use super::PostgresPool;

/// What each run has got to, shared by every replica.
#[derive(Debug, Clone)]
pub struct PostgresAgenticSystemExecutions {
    pool: PostgresPool,
}

impl PostgresAgenticSystemExecutions {
    #[must_use]
    pub const fn new(pool: PostgresPool) -> Self {
        Self { pool }
    }

    fn inner(&self) -> &PgPool {
        self.pool.inner()
    }
}

#[async_trait]
impl AgenticSystemExecutionStorePort for PostgresAgenticSystemExecutions {
    async fn create(
        &self,
        execution: AgenticSystemExecution,
    ) -> Result<AgenticSystemExecutionCreation, DomainError> {
        let inserted = sqlx::query(
            "INSERT INTO agentic_system_executions \
             (execution_id, system_id, updated_at, payload) \
             VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
        )
        .bind(execution.id().as_str())
        .bind(execution.system().id().as_str())
        .bind(execution.updated_at())
        .bind(encode(&execution, "encode agentic system execution")?)
        .execute(self.inner())
        .await
        .map_err(|error| sqlx_error(error, "open agentic system execution"))?;
        if inserted.rows_affected() == 1 {
            return Ok(AgenticSystemExecutionCreation::Created(execution));
        }
        let existing = self
            .get(execution.id())
            .await?
            .ok_or(DomainError::NotFound {
                what: "agentic_system_execution",
            })?;
        Ok(AgenticSystemExecutionCreation::AlreadyExists(existing))
    }

    async fn get(
        &self,
        id: &AgenticSystemExecutionId,
    ) -> Result<Option<AgenticSystemExecution>, DomainError> {
        let row =
            sqlx::query("SELECT payload FROM agentic_system_executions WHERE execution_id = $1")
                .bind(id.as_str())
                .fetch_optional(self.inner())
                .await
                .map_err(|error| sqlx_error(error, "read agentic system execution"))?;
        row.as_ref().map(payload).transpose()
    }

    /// The write carries its own precondition: the row moves only when
    /// `updated_at` is still what the caller read, so a lost race is a
    /// zero-row update rather than an overwrite.
    async fn update(
        &self,
        execution: AgenticSystemExecution,
        expected_updated_at: OffsetDateTime,
    ) -> Result<AgenticSystemExecutionUpdate, DomainError> {
        let updated = sqlx::query(
            "UPDATE agentic_system_executions SET updated_at = $1, payload = $2 \
             WHERE execution_id = $3 AND updated_at = $4",
        )
        .bind(execution.updated_at())
        .bind(encode(&execution, "encode agentic system execution")?)
        .bind(execution.id().as_str())
        .bind(expected_updated_at)
        .execute(self.inner())
        .await
        .map_err(|error| sqlx_error(error, "advance agentic system execution"))?;
        if updated.rows_affected() == 1 {
            return Ok(AgenticSystemExecutionUpdate::Updated(execution));
        }
        let stored = self
            .get(execution.id())
            .await?
            .ok_or(DomainError::NotFound {
                what: "agentic_system_execution",
            })?;
        Ok(AgenticSystemExecutionUpdate::Conflict(stored))
    }

    async fn list_by_system(
        &self,
        id: &AgenticSystemId,
    ) -> Result<Vec<AgenticSystemExecution>, DomainError> {
        let rows = sqlx::query(
            "SELECT payload FROM agentic_system_executions WHERE system_id = $1 \
             ORDER BY execution_id",
        )
        .bind(id.as_str())
        .fetch_all(self.inner())
        .await
        .map_err(|error| sqlx_error(error, "list agentic system executions"))?;
        let mut found = rows
            .iter()
            .map(payload)
            .collect::<Result<Vec<AgenticSystemExecution>, _>>()?;
        found.sort_by_key(AgenticSystemExecution::created_at);
        Ok(found)
    }
}

fn payload(row: &sqlx::postgres::PgRow) -> Result<AgenticSystemExecution, DomainError> {
    let bytes: Vec<u8> = row
        .try_get("payload")
        .map_err(|error| sqlx_error(error, "read agentic system execution payload"))?;
    decode(&bytes, "decode agentic system execution")
}
