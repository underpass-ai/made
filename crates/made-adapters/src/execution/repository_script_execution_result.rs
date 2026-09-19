use made_core::error::DomainError;
use made_core::ports::CeremonyExecutionObservation;
use made_core::value_objects::{
    ExecutionIntent, ExecutionOperationId, ExecutionRequestDigest, StepClaimFence, StepResult,
    StepStatus,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Durable result written by an authorized repository script.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct RepositoryScriptExecutionResult {
    operation_id: ExecutionOperationId,
    request_digest: ExecutionRequestDigest,
    producer_claim_fence: StepClaimFence,
    result: StepResult,
    #[serde(with = "time::serde::rfc3339")]
    observed_at: OffsetDateTime,
}

impl RepositoryScriptExecutionResult {
    pub(super) fn validate(&self, intent: &ExecutionIntent) -> Result<(), DomainError> {
        intent.validate()?;
        if &self.operation_id != intent.operation().operation_id()
            || &self.request_digest != intent.operation().request_digest()
        {
            return Err(DomainError::Conflict {
                what: "repository_script_execution_result",
            });
        }
        if matches!(
            self.result.status(),
            StepStatus::Pending | StepStatus::InProgress
        ) || (self.result.status() == StepStatus::Failed
            && self.result.error_message().is_none())
            || (self.result.status() != StepStatus::Failed && self.result.error_message().is_some())
        {
            return Err(DomainError::InvariantViolated {
                reason: "repository script returned a non-terminal step result",
            });
        }
        Ok(())
    }

    pub(super) fn into_observation(self) -> CeremonyExecutionObservation {
        CeremonyExecutionObservation::new(
            self.producer_claim_fence,
            None,
            self.result,
            Vec::new(),
            self.observed_at,
        )
    }
}
