use async_trait::async_trait;
use made_core::entities::Proposal;
use made_core::error::DomainError;
use made_core::ports::ExecutorPort;
use made_core::value_objects::{Attributes, ExecutionOutcome};

/// Honest default for orchestration when a host configured no executor.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct UnconfiguredExecutor;

#[async_trait]
impl ExecutorPort for UnconfiguredExecutor {
    async fn execute(
        &self,
        _winner: &Proposal,
        _options: &Attributes,
    ) -> Result<ExecutionOutcome, DomainError> {
        Err(DomainError::InvariantViolated {
            reason: "embedded executor is not configured",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use made_core::value_objects::{AgentId, ProposalId, Specialty};
    use time::macros::datetime;

    #[tokio::test]
    async fn rejects_execution_with_an_actionable_reason() {
        let proposal = Proposal::new(
            ProposalId::new("proposal-1").unwrap(),
            AgentId::new("agent-1").unwrap(),
            Specialty::new("triage").unwrap(),
            "candidate",
            Attributes::empty(),
            datetime!(2026-09-18 12:00:00 UTC),
        )
        .unwrap();

        let error = UnconfiguredExecutor
            .execute(&proposal, &Attributes::empty())
            .await
            .unwrap_err();
        assert_eq!(
            error,
            DomainError::InvariantViolated {
                reason: "embedded executor is not configured",
            }
        );
    }
}
