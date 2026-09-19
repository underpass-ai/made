//! [`AgentRegistryPort`] — write side of the agent registry.
//!
//! The read side is [`AgentResolverPort`](super::AgentResolverPort):
//! use cases that need live handles go through the resolver. This port
//! exists so operator-facing RPCs (`RegisterAgent`, `UnregisterAgent`)
//! can mutate the registry without coupling to a concrete adapter.
//!
//! Kept segregated from the resolver per ISP: the deliberation use
//! case does not need write access, and an adapter may reasonably
//! implement read but not write (e.g. a cache in front of a static
//! config file).

use std::sync::Arc;

use async_trait::async_trait;

use crate::error::DomainError;
use crate::ports::agent::AgentPort;
use crate::ports::AgentDescriptor;
use crate::value_objects::AgentId;

#[async_trait]
pub trait AgentRegistryPort: Send + Sync {
    /// Register `agent` under its [`AgentPort::id`]. Returns
    /// [`DomainError::AlreadyExists`] if the id is taken.
    async fn register(&self, agent: Arc<dyn AgentPort>) -> Result<(), DomainError>;

    /// Register the original, non-secret construction descriptor with its live handle.
    /// Durable implementations preserve the descriptor; in-memory registries only
    /// need the handle. The two identities must agree before storage is changed.
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
        self.register(agent).await
    }

    /// Unregister the agent with `id`. Returns
    /// [`DomainError::NotFound`] when no such agent is present.
    async fn unregister(&self, id: &AgentId) -> Result<(), DomainError>;
}
