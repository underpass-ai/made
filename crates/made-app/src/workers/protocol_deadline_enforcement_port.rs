use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::value_objects::{ProtocolId, StepClaimFence, StepId};

use crate::usecases::{EnforceProtocolDeadlinesInput, EnforceProtocolDeadlinesUseCase};

/// Enforces absolute deadlines and returns the claim still current afterwards.
#[async_trait]
pub trait ProtocolDeadlineEnforcementPort: Send + Sync {
    async fn enforce_current_claim(
        &self,
        protocol_id: &ProtocolId,
        step_id: &StepId,
    ) -> Result<Option<StepClaimFence>, DomainError>;
}

#[async_trait]
impl ProtocolDeadlineEnforcementPort for EnforceProtocolDeadlinesUseCase {
    async fn enforce_current_claim(
        &self,
        protocol_id: &ProtocolId,
        step_id: &StepId,
    ) -> Result<Option<StepClaimFence>, DomainError> {
        let instance = self
            .execute(EnforceProtocolDeadlinesInput::new(protocol_id.clone()))
            .await?;
        Ok(instance.step_claim_fence(step_id).ok())
    }
}
