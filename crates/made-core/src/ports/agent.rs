//! [`AgentPort`] — provider-agnostic interface for deliberation agents.
//!
//! MADE does not know or care whether an agent is backed
//! by a local vLLM server, an Anthropic or OpenAI API, a deterministic
//! rule engine, or a human in the loop. Every agent implementation
//! lives behind this trait; no provider is privileged.
//!
//! The three methods mirror the peer-deliberation algorithm in the
//! reference implementation: an agent can **propose**, **critique** a
//! peer's proposal, and **revise** its own proposal given feedback.

use async_trait::async_trait;

use crate::error::DomainError;
use crate::ports::{Critique, DraftRequest, Revision};
use crate::value_objects::{AgentId, ProposalContent, Specialty};

#[async_trait]
pub trait AgentPort: std::fmt::Debug + Send + Sync {
    /// Stable identity of this agent. Used for attribution and logging.
    fn id(&self) -> &AgentId;

    /// Specialty the agent claims expertise in. Must match the council
    /// specialty at registration time.
    fn specialty(&self) -> &Specialty;

    /// Produce an initial proposal for a task.
    async fn generate(&self, request: DraftRequest) -> Result<Revision, DomainError>;

    /// Produce a critique of a peer's proposal content.
    async fn critique(
        &self,
        peer_content: &ProposalContent,
        constraints: &crate::entities::TaskConstraints,
    ) -> Result<Critique, DomainError>;

    /// Produce a revised content given a peer's critique.
    async fn revise(
        &self,
        own_content: &ProposalContent,
        critique: &Critique,
    ) -> Result<Revision, DomainError>;
}
