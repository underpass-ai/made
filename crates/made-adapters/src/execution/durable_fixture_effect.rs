use made_core::error::DomainError;
use made_core::value_objects::{
    ExecutionIntent, ExecutionOperationId, ExecutionRequestDigest, StepClaimFence, StepResult,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Durable record written by the reference fixture connector as its external effect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct DurableFixtureEffect {
    operation_id: ExecutionOperationId,
    request_digest: ExecutionRequestDigest,
    producer_claim_fence: StepClaimFence,
    result: StepResult,
    #[serde(with = "time::serde::rfc3339")]
    observed_at: OffsetDateTime,
}

impl DurableFixtureEffect {
    pub(super) fn new(
        intent: &ExecutionIntent,
        result: StepResult,
        observed_at: OffsetDateTime,
    ) -> Self {
        Self {
            operation_id: intent.operation().operation_id().clone(),
            request_digest: intent.operation().request_digest().clone(),
            producer_claim_fence: intent.claim_fence().clone(),
            result,
            observed_at,
        }
    }

    pub(super) fn validate(&self, intent: &ExecutionIntent) -> Result<(), DomainError> {
        if &self.operation_id != intent.operation().operation_id()
            || &self.request_digest != intent.operation().request_digest()
        {
            return Err(DomainError::Conflict {
                what: "durable_fixture_effect",
            });
        }
        Ok(())
    }

    pub(super) fn into_parts(self) -> (StepClaimFence, StepResult, OffsetDateTime) {
        (self.producer_claim_fence, self.result, self.observed_at)
    }
}
