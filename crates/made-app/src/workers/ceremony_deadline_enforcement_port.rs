use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::value_objects::{CeremonyId, StepClaimFence, StepId};

use crate::usecases::{EnforceCeremonyDeadlinesInput, EnforceCeremonyDeadlinesUseCase};

/// Enforces absolute deadlines and returns the claim still current afterwards.
#[async_trait]
pub trait CeremonyDeadlineEnforcementPort: Send + Sync {
    async fn enforce_current_claim(
        &self,
        ceremony_id: &CeremonyId,
        step_id: &StepId,
    ) -> Result<Option<StepClaimFence>, DomainError>;
}

#[async_trait]
impl CeremonyDeadlineEnforcementPort for EnforceCeremonyDeadlinesUseCase {
    async fn enforce_current_claim(
        &self,
        ceremony_id: &CeremonyId,
        step_id: &StepId,
    ) -> Result<Option<StepClaimFence>, DomainError> {
        let instance = self
            .execute(EnforceCeremonyDeadlinesInput::new(ceremony_id.clone()))
            .await?;
        Ok(instance.step_claim_fence(step_id).ok())
    }
}
