use time::OffsetDateTime;

use crate::value_objects::{
    ArtifactRef, ExternalOperationId, MeasuredBudgetQuantities, StepClaimFence, StepResult,
};

/// Terminal observation returned by an execution connector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyExecutionObservation {
    producer_claim_fence: StepClaimFence,
    external_operation_id: Option<ExternalOperationId>,
    result: StepResult,
    artifacts: Vec<ArtifactRef>,
    budget_measurement: MeasuredBudgetQuantities,
    observed_at: OffsetDateTime,
}

impl CeremonyExecutionObservation {
    #[must_use]
    pub fn new(
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
            budget_measurement: MeasuredBudgetQuantities::default(),
            observed_at,
        }
    }

    #[must_use]
    pub fn with_budget_measurement(mut self, measured: MeasuredBudgetQuantities) -> Self {
        self.budget_measurement = measured;
        self
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
    pub const fn budget_measurement(&self) -> MeasuredBudgetQuantities {
        self.budget_measurement
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        StepClaimFence,
        Option<ExternalOperationId>,
        StepResult,
        Vec<ArtifactRef>,
        MeasuredBudgetQuantities,
        OffsetDateTime,
    ) {
        (
            self.producer_claim_fence,
            self.external_operation_id,
            self.result,
            self.artifacts,
            self.budget_measurement,
            self.observed_at,
        )
    }
}
