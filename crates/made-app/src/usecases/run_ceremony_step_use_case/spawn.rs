use made_core::entities::CeremonyInstance;
use made_core::error::DomainError;
use made_core::value_objects::{AuditActorKind, StepAttempt, StepClaimFence, StepId};

use super::{RunCeremonyStepOutput, RunCeremonyStepUseCase};
use crate::usecases::PrepareCeremonyChildrenInput;

impl RunCeremonyStepUseCase {
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
