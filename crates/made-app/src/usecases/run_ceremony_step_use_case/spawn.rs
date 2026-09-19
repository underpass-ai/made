use made_core::entities::CeremonyInstance;
use made_core::error::DomainError;
use made_core::value_objects::{AuditActorKind, StepAttempt, StepClaimFence, StepId};

use super::{RunCeremonyStepOutput, RunCeremonyStepUseCase};
use crate::usecases::{PrepareCeremonyChildrenInput, StartCeremonyStepOutput};

impl RunCeremonyStepUseCase {
    pub async fn execute_claimed_spawn(
        &self,
        claim: &StartCeremonyStepOutput,
        step_id: StepId,
        actor_kind: AuditActorKind,
    ) -> Result<RunCeremonyStepOutput, DomainError> {
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
        instance: CeremonyInstance,
        step_id: StepId,
        claim_fence: StepClaimFence,
        actor_kind: AuditActorKind,
        attempt: StepAttempt,
    ) -> Result<RunCeremonyStepOutput, DomainError> {
        let children = self
            .children
            .as_ref()
            .expect("child orchestrator checked before the durable claim");
        let output = children
            .execute(PrepareCeremonyChildrenInput::new(
                instance.id().clone(),
                step_id,
                claim_fence,
                actor_kind,
            ))
            .await?;
        let (instance, result) = output.into_parts();
        Ok(RunCeremonyStepOutput::new(instance, attempt, result))
    }
}
