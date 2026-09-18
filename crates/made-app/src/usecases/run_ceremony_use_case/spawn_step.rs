use made_core::error::DomainError;
use made_core::value_objects::AuditActorKind;

use crate::usecases::PrepareCeremonyChildrenInput;

use super::{claimed_step::ClaimedStep, run_step_output::RunStepOutput, RunCeremonyUseCase};

impl RunCeremonyUseCase {
    pub(super) async fn execute_spawn_claimed_step(
        &self,
        claimed: ClaimedStep,
        actor_kind: AuditActorKind,
    ) -> Result<RunStepOutput, DomainError> {
        let children = self
            .children
            .as_ref()
            .ok_or(DomainError::InvariantViolated {
                reason: "child-spawning ceremony requires the child orchestrator",
            })?;
        let output = children
            .execute(PrepareCeremonyChildrenInput::new(
                claimed.request.instance_id().clone(),
                claimed.step_id.clone(),
                claimed.claim_fence.clone(),
                actor_kind,
            ))
            .await?;
        let session = self.stream.load(output.instance().id()).await?;
        Ok(RunStepOutput {
            session,
            step_id: claimed.step_id,
            role_id: claimed.role_id,
            state_visit: claimed.state_visit,
            state_iteration: claimed.state_iteration,
            iteration: claimed.iteration,
            attempt: claimed.attempt,
            result: output.result().clone(),
        })
    }
}
