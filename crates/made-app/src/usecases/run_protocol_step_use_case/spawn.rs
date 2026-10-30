use made_core::entities::ProtocolInstance;
use made_core::error::DomainError;
use made_core::value_objects::{AuditActorKind, StepAttempt, StepClaimFence, StepId};

use super::{RunProtocolStepOutput, RunProtocolStepUseCase};
use crate::usecases::{PrepareProtocolChildrenInput, StartProtocolStepOutput};

impl RunProtocolStepUseCase {
    pub async fn execute_claimed_spawn(
        &self,
        claim: &StartProtocolStepOutput,
        step_id: StepId,
        actor_kind: AuditActorKind,
    ) -> Result<RunProtocolStepOutput, DomainError> {
        let claim_fence = claim.instance().step_claim_fence(&step_id)?;
        if &claim_fence != claim.claim_fence() {
            return Err(DomainError::InvariantViolated {
                reason: "accepted spawn claim fence differs from its accepted instance",
            });
        }
        self.execute_spawn_step(
            claim.instance().clone(),
            step_id,
            claim_fence,
            actor_kind,
            claim.attempt(),
        )
        .await
    }

    pub(super) async fn execute_spawn_step(
        &self,
        instance: ProtocolInstance,
        step_id: StepId,
        claim_fence: StepClaimFence,
        actor_kind: AuditActorKind,
        attempt: StepAttempt,
    ) -> Result<RunProtocolStepOutput, DomainError> {
        let children = self
            .children
            .as_ref()
            .expect("child orchestrator checked before the durable claim");
        let output = children
            .execute(PrepareProtocolChildrenInput::new(
                instance.id().clone(),
                step_id,
                claim_fence,
                actor_kind,
            ))
            .await?;
        let (instance, result) = output.into_parts();
        Ok(RunProtocolStepOutput::new(instance, attempt, result))
    }
}
