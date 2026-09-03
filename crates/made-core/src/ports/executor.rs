//! [`ExecutorPort`] — opaque execution of a winning proposal.
//!
//! After a deliberation produces a winner, the `Orchestrate` use case
//! optionally hands the proposal off to an executor (e.g. the Underpass
//! Runtime via gRPC, a local subprocess, a human-in-the-loop queue,
//! …). MADE does not know what execution means beyond
//! "adapter runs it and reports an outcome".

use crate::entities::Proposal;
use crate::error::DomainError;
use crate::value_objects::{Attributes, ExecutionOutcome};
use async_trait::async_trait;

#[async_trait]
pub trait ExecutorPort: Send + Sync {
    /// Execute the winning proposal. Adapters are free to block or
    /// stream, as long as the final outcome is returned.
    async fn execute(
        &self,
        winner: &Proposal,
        options: &Attributes,
    ) -> Result<ExecutionOutcome, DomainError>;
}
