use time::OffsetDateTime;

use crate::value_objects::{ArtifactRef, ExternalOperationId, StepClaimFence, StepResult};

/// Terminal observation returned by an execution connector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyExecutionObservation {
    producer_claim_fence: StepClaimFence,
    external_operation_id: Option<ExternalOperationId>,
    result: StepResult,
    artifacts: Vec<ArtifactRef>,
    observed_at: OffsetDateTime,
}

impl CeremonyExecutionObservation {
    #[must_use]
    pub const fn new(
        producer_claim_fence: StepClaimFence,
        external_operation_id: Option<ExternalOperationId>,
        result: StepResult,
        artifacts: Vec<ArtifactRef>,
        observed_at: OffsetDateTime,
    ) -> Self {
        Self {
            producer_claim_fence,
            external_operation_id,
            result,
            artifacts,
            observed_at,
        }
    }

    #[must_use]
    pub const fn producer_claim_fence(&self) -> &StepClaimFence {
        &self.producer_claim_fence
    }

    #[must_use]
    pub const fn external_operation_id(&self) -> Option<&ExternalOperationId> {
        self.external_operation_id.as_ref()
    }

    #[must_use]
    pub const fn result(&self) -> &StepResult {
        &self.result
    }

    #[must_use]
    pub fn artifacts(&self) -> &[ArtifactRef] {
        &self.artifacts
    }

    #[must_use]
    pub const fn observed_at(&self) -> OffsetDateTime {
        self.observed_at
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        StepClaimFence,
        Option<ExternalOperationId>,
        StepResult,
        Vec<ArtifactRef>,
        OffsetDateTime,
    ) {
        (
            self.producer_claim_fence,
            self.external_operation_id,
            self.result,
            self.artifacts,
            self.observed_at,
        )
    }
}
