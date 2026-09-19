use super::SqliteCouncilStore;
use crate::engine::Table;
use async_trait::async_trait;
use made_core::entities::CouncilJournalEvent;
use made_core::error::DomainError;
use made_core::ports::{
    AgentDescriptor, AgentFactoryPort, AgentPort, AgentRegistryPort, AgentResolverPort,
};
use made_core::value_objects::AgentId;
use std::sync::Arc;

#[derive(Clone)]
pub struct SqliteAgentRegistry {
    store: SqliteCouncilStore,
    factory: Arc<dyn AgentFactoryPort>,
}
impl std::fmt::Debug for SqliteAgentRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteAgentRegistry")
            .finish_non_exhaustive()
    }
}
impl SqliteAgentRegistry {
    #[must_use]
    pub fn new(store: SqliteCouncilStore, factory: Arc<dyn AgentFactoryPort>) -> Self {
        Self { store, factory }
    }
    pub async fn descriptor(&self, id: &AgentId) -> Result<AgentDescriptor, DomainError> {
        self.store
            .get(Table::CouncilAgents, id.to_string(), "agent")
            .await
    }
}
#[async_trait]
impl AgentRegistryPort for SqliteAgentRegistry {
    async fn register(&self, _agent: Arc<dyn AgentPort>) -> Result<(), DomainError> {
        Err(DomainError::InvariantViolated {
            reason: "durable agent registration requires the original descriptor",
        })
    }
    async fn register_described(
        &self,
        descriptor: AgentDescriptor,
        agent: Arc<dyn AgentPort>,
    ) -> Result<(), DomainError> {
        if descriptor.id != *agent.id() || descriptor.specialty != *agent.specialty() {
            return Err(DomainError::InvariantViolated {
                reason: "agent descriptor and materialized identity differ",
            });
        }
        crate::persisted_agent_descriptor::validate(&descriptor)?;
        self.store
            .insert(
                Table::CouncilAgents,
                descriptor.id.to_string(),
                descriptor.clone(),
                "agent",
                CouncilJournalEvent::AgentRegistered(descriptor),
            )
            .await
    }
    async fn unregister(&self, id: &AgentId) -> Result<(), DomainError> {
        self.store
            .delete(
                Table::CouncilAgents,
                id.to_string(),
                "agent",
                CouncilJournalEvent::AgentUnregistered(id.clone()),
            )
            .await
    }
}
#[async_trait]
impl AgentResolverPort for SqliteAgentRegistry {
    async fn resolve(&self, id: &AgentId) -> Result<Arc<dyn AgentPort>, DomainError> {
        let descriptor = self.descriptor(id).await?;
        crate::persisted_agent_descriptor::validate(&descriptor)?;
        let expected_specialty = descriptor.specialty.clone();
        let agent = self.factory.create(descriptor).await?;
        if agent.id() != id || agent.specialty() != &expected_specialty {
            return Err(DomainError::InvariantViolated {
                reason: "agent descriptor and materialized identity differ",
            });
        }
        Ok(agent)
    }
}
