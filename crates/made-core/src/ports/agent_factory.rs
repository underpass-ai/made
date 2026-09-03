//! [`AgentFactoryPort`] — materialize an [`AgentPort`] from a typed descriptor.
//!
//! Callers (the composition root and the `RegisterAgent` RPC) do not
//! construct agent implementations directly; they hand a descriptor to
//! this factory and receive a live handle back. That keeps the set of
//! wired providers (vLLM, Anthropic, OpenAI, rule-based, human-in-the-
//! loop, …) behind Cargo features at the adapter layer, exactly like
//! every other provider-specific concern in this crate.
//!
//! The `kind` field in [`AgentDescriptor`] names the provider the
//! factory should dispatch to. Adapters return
//! [`DomainError::InvariantViolated`] when asked for a kind they do
//! not support; the composition root is responsible for wiring a
//! factory that recognises every kind the deployment intends to accept.

use std::sync::Arc;

use crate::error::DomainError;
use crate::ports::agent::AgentPort;
use crate::ports::AgentDescriptor;
use async_trait::async_trait;

#[async_trait]
pub trait AgentFactoryPort: Send + Sync {
    /// Produce a live agent matching `descriptor`.
    ///
    /// Adapters that do not recognise `descriptor.kind` must return
    /// [`DomainError::InvariantViolated`] with a reason naming the
    /// unsupported kind, so operators see the mismatch loudly.
    async fn create(&self, descriptor: AgentDescriptor) -> Result<Arc<dyn AgentPort>, DomainError>;
}
