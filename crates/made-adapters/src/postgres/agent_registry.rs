//! Postgres implementation of [`AgentRegistryPort`] + [`AgentResolverPort`].
//!
//! Rows persist the typed [`AgentDescriptor`] (id, specialty, kind,
//! attributes). On read, descriptors are rehydrated into live
//! `Arc<dyn AgentPort>` handles via the wired [`AgentFactoryPort`];
//! no pickled provider state crosses the database boundary. That keeps
//! the registry content portable across replicas — each replica can
//! materialize its own live handles using whatever factory it has
//! feature-enabled.
//!
//! Registration preserves the original descriptor. A live handle alone cannot
//! prove which provider constructed it and is therefore not a durable record.

use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::CouncilJournalEvent;
use made_core::error::DomainError;
use made_core::ports::{
    AgentDescriptor, AgentFactoryPort, AgentPort, AgentRegistryPort, AgentResolverPort,
};
use made_core::value_objects::{AgentId, AgentKind, Attributes, AuthorizationEvidence, Specialty};
use serde_json::Value as JsonValue;
use sqlx::Row;

use super::error::{serde_to_domain, sqlx_to_domain};
use super::pool::PostgresPool;

#[derive(Clone)]
pub struct PostgresAgentRegistry {
    pool: PostgresPool,
    factory: Arc<dyn AgentFactoryPort>,
}

impl std::fmt::Debug for PostgresAgentRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PostgresAgentRegistry").finish()
    }
}

impl PostgresAgentRegistry {
    #[must_use]
    pub fn new(pool: PostgresPool, factory: Arc<dyn AgentFactoryPort>) -> Self {
        Self { pool, factory }
    }

    /// Persist validated provider construction options; credentials remain host-side.
    pub async fn insert_descriptor(&self, descriptor: &AgentDescriptor) -> Result<(), DomainError> {
        self.insert_descriptor_authorized(descriptor, None).await
    }

    async fn insert_descriptor_authorized(
        &self,
        descriptor: &AgentDescriptor,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<(), DomainError> {
        let mut tx = super::council_journal_store::begin(&self.pool).await?;
        crate::persisted_agent_descriptor::validate(descriptor)?;
        let attributes: JsonValue = serde_json::to_value(&descriptor.attributes)
            .map_err(|e| serde_to_domain(&e, "insert_descriptor"))?;
        let result = sqlx::query(
            "
            INSERT INTO agents (agent_id, specialty, kind, attributes)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (agent_id) DO NOTHING
            ",
        )
        .bind(descriptor.id.as_str())
        .bind(descriptor.specialty.as_str())
        .bind(descriptor.kind.as_str())
        .bind(&attributes)
        .execute(&mut *tx)
        .await
        .map_err(|e| sqlx_to_domain(e, "insert_descriptor"))?;
        if result.rows_affected() == 0 {
            return Err(DomainError::AlreadyExists { what: "agent" });
        }
        super::council_journal_store::append(
            &mut tx,
            CouncilJournalEvent::AgentRegistered(descriptor.clone()),
            authorization,
        )
        .await?;
        tx.commit()
            .await
            .map_err(|e| sqlx_to_domain(e, "commit council mutation"))
    }

    async fn load_descriptor(&self, id: &AgentId) -> Result<Option<AgentDescriptor>, DomainError> {
        let row = sqlx::query(
            "SELECT agent_id, specialty, kind, attributes FROM agents WHERE agent_id = $1",
        )
        .bind(id.as_str())
        .fetch_optional(self.pool.inner())
        .await
        .map_err(|e| sqlx_to_domain(e, "resolve"))?;
        let Some(row) = row else {
            return Ok(None);
        };
        Ok(Some(descriptor_from_row(&row)?))
    }
}

#[async_trait]
impl AgentRegistryPort for PostgresAgentRegistry {
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
        self.register_described_authorized(descriptor, agent, None)
            .await
    }

    async fn register_described_authorized(
        &self,
        descriptor: AgentDescriptor,
        agent: Arc<dyn AgentPort>,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<(), DomainError> {
        if descriptor.id != *agent.id() || descriptor.specialty != *agent.specialty() {
            return Err(DomainError::InvariantViolated {
                reason: "agent descriptor and materialized identity differ",
            });
        }
        self.insert_descriptor_authorized(&descriptor, authorization)
            .await
    }

    async fn unregister(&self, id: &AgentId) -> Result<(), DomainError> {
        self.unregister_authorized(id, None).await
    }

    async fn unregister_authorized(
        &self,
        id: &AgentId,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<(), DomainError> {
        let mut tx = super::council_journal_store::begin(&self.pool).await?;
        let result = sqlx::query("DELETE FROM agents WHERE agent_id = $1")
            .bind(id.as_str())
            .execute(&mut *tx)
            .await
            .map_err(|e| sqlx_to_domain(e, "unregister"))?;
        if result.rows_affected() == 0 {
            return Err(DomainError::NotFound { what: "agent" });
        }
        super::council_journal_store::append(
            &mut tx,
            CouncilJournalEvent::AgentUnregistered(id.clone()),
            authorization,
        )
        .await?;
        tx.commit()
            .await
            .map_err(|e| sqlx_to_domain(e, "commit council mutation"))
    }
}

#[async_trait]
impl AgentResolverPort for PostgresAgentRegistry {
    async fn resolve(&self, id: &AgentId) -> Result<Arc<dyn AgentPort>, DomainError> {
        let descriptor = self
            .load_descriptor(id)
            .await?
            .ok_or(DomainError::NotFound { what: "agent" })?;
        self.factory.create(descriptor).await
    }
}

fn descriptor_from_row(row: &sqlx::postgres::PgRow) -> Result<AgentDescriptor, DomainError> {
    let agent_id: String = row
        .try_get("agent_id")
        .map_err(|e| sqlx_to_domain(e, "resolve"))?;
    let specialty: String = row
        .try_get("specialty")
        .map_err(|e| sqlx_to_domain(e, "resolve"))?;
    let kind: String = row
        .try_get("kind")
        .map_err(|e| sqlx_to_domain(e, "resolve"))?;
    let attrs_value: JsonValue = row
        .try_get("attributes")
        .map_err(|e| sqlx_to_domain(e, "resolve"))?;
    let attributes: Attributes =
        serde_json::from_value(attrs_value).map_err(|e| serde_to_domain(&e, "resolve"))?;
    Ok(AgentDescriptor {
        id: AgentId::new(agent_id)?,
        specialty: Specialty::new(specialty)?,
        kind: AgentKind::new(kind)?,
        attributes,
    })
}
