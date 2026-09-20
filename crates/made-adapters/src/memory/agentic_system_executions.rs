use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::AgenticSystemExecution;
use made_core::error::DomainError;
use made_core::ports::{
    AgenticSystemExecutionCreation, AgenticSystemExecutionStorePort, AgenticSystemExecutionUpdate,
};
use made_core::value_objects::{AgenticSystemExecutionId, AgenticSystemId};
use time::OffsetDateTime;
use tokio::sync::RwLock;

/// Process-local runs, one row per run.
#[derive(Debug, Default, Clone)]
pub struct InMemoryAgenticSystemExecutions {
    inner: Arc<RwLock<BTreeMap<AgenticSystemExecutionId, AgenticSystemExecution>>>,
}

impl InMemoryAgenticSystemExecutions {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl AgenticSystemExecutionStorePort for InMemoryAgenticSystemExecutions {
    async fn create(
        &self,
        execution: AgenticSystemExecution,
    ) -> Result<AgenticSystemExecutionCreation, DomainError> {
        let mut runs = self.inner.write().await;
        if let Some(existing) = runs.get(execution.id()) {
            return Ok(AgenticSystemExecutionCreation::AlreadyExists(
                existing.clone(),
            ));
        }
        runs.insert(execution.id().clone(), execution.clone());
        Ok(AgenticSystemExecutionCreation::Created(execution))
    }

    async fn get(
        &self,
        id: &AgenticSystemExecutionId,
    ) -> Result<Option<AgenticSystemExecution>, DomainError> {
        Ok(self.inner.read().await.get(id).cloned())
    }

    async fn update(
        &self,
        execution: AgenticSystemExecution,
        expected_updated_at: OffsetDateTime,
    ) -> Result<AgenticSystemExecutionUpdate, DomainError> {
        let mut runs = self.inner.write().await;
        let stored = runs.get(execution.id()).ok_or(DomainError::NotFound {
            what: "agentic_system_execution",
        })?;
        if stored.updated_at() != expected_updated_at {
            return Ok(AgenticSystemExecutionUpdate::Conflict(stored.clone()));
        }
        runs.insert(execution.id().clone(), execution.clone());
        Ok(AgenticSystemExecutionUpdate::Updated(execution))
    }

    async fn list_by_system(
        &self,
        id: &AgenticSystemId,
    ) -> Result<Vec<AgenticSystemExecution>, DomainError> {
        let runs = self.inner.read().await;
        let mut found: Vec<AgenticSystemExecution> = runs
            .values()
            .filter(|execution| execution.system().id() == id)
            .cloned()
            .collect();
        found.sort_by_key(AgenticSystemExecution::created_at);
        Ok(found)
    }
}
