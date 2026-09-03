//! No-op [`ExecutorPort`].
//!
//! Returns a successful outcome with zero duration and empty output
//! for every call. Useful when a deployment does not intend to
//! execute winning proposals (deliberation-only mode) or in tests.

use async_trait::async_trait;
use made_core::entities::Proposal;
use made_core::error::DomainError;
use made_core::ports::ExecutorPort;
use made_core::value_objects::{
    Attributes, DurationMs, ExecutionId, ExecutionOutcome, ExecutionStatus,
};
use tracing::debug;
use uuid::Uuid;

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopExecutor;

impl NoopExecutor {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ExecutorPort for NoopExecutor {
    async fn execute(
        &self,
        winner: &Proposal,
        _options: &Attributes,
    ) -> Result<ExecutionOutcome, DomainError> {
        debug!(
            proposal_id = winner.id().as_str(),
            "noop executor: skipping execution"
        );
        Ok(ExecutionOutcome::new(
            ExecutionId::new(Uuid::new_v4().to_string())?,
            ExecutionStatus::Succeeded,
            DurationMs::ZERO,
            Attributes::empty(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use made_core::value_objects::{AgentId, ProposalId, Specialty};
    use time::macros::datetime;

    fn proposal() -> Proposal {
        Proposal::new(
            ProposalId::new("p1").unwrap(),
            AgentId::new("a").unwrap(),
            Specialty::new("s").unwrap(),
            "content",
            Attributes::empty(),
            datetime!(2026-04-15 12:00:00 UTC),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn always_succeeds() {
        let out = NoopExecutor::new()
            .execute(&proposal(), &Attributes::empty())
            .await
            .unwrap();
        assert!(out.status().is_success());
        assert_eq!(out.duration(), DurationMs::ZERO);
        assert!(!out.id().as_str().is_empty());
    }

    #[tokio::test]
    async fn execution_ids_are_unique_across_calls() {
        let exec = NoopExecutor::new();
        let a = exec
            .execute(&proposal(), &Attributes::empty())
            .await
            .unwrap();
        let b = exec
            .execute(&proposal(), &Attributes::empty())
            .await
            .unwrap();
        assert_ne!(a.id(), b.id());
    }
}
