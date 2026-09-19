use made_app::budgets::{BudgetedStepClaimInput, BudgetedStepClaimUseCase};
use made_app::usecases::{RunCeremonyStepInput, RunCeremonyStepOutput, RunCeremonyStepUseCase};
use made_core::value_objects::{AuditActorKind, AuthorizationAction, StepId};
use made_core::{BudgetError, DomainError};

use super::EmbeddedMade;

/// Narrow adapter collaborator for the compound child-preparation operation.
#[derive(Clone, Debug)]
pub struct EmbeddedCeremonyOperationAuthority {
    engine: EmbeddedMade,
}

impl EmbeddedCeremonyOperationAuthority {
    #[must_use]
    pub fn for_engine(engine: &EmbeddedMade) -> Self {
        Self {
            engine: engine.clone(),
        }
    }

    pub async fn prepare_unbudgeted_children(
        &self,
        input: RunCeremonyStepInput,
    ) -> Result<RunCeremonyStepOutput, DomainError> {
        self.engine.require_authorized_ceremony_action(
            AuthorizationAction::PrepareCeremonyChildren,
            input.instance_id(),
        )?;
        Box::pin(self.run_step().execute(input)).await
    }

    pub async fn prepare_budgeted_children(
        &self,
        input: BudgetedStepClaimInput,
        step_id: StepId,
        actor_kind: AuditActorKind,
    ) -> Result<RunCeremonyStepOutput, BudgetError> {
        self.engine.require_authorized_ceremony_action(
            AuthorizationAction::PrepareCeremonyChildren,
            input.ceremony_id(),
        )?;
        let claim = BudgetedStepClaimUseCase::new(
            self.engine.resolve_definition(),
            self.engine.stream.clone(),
            self.engine.clock.clone(),
            self.engine.budgets.clone(),
        )
        .with_max_parallel_ceiling(self.engine.max_parallel_ceiling)
        .execute(input)
        .await?;
        self.run_step()
            .execute_claimed_spawn(claim.claim(), step_id, actor_kind)
            .await
            .map_err(Into::into)
    }

    fn run_step(&self) -> RunCeremonyStepUseCase {
        RunCeremonyStepUseCase::new(
            self.engine.resolve_definition(),
            self.engine.stream.clone(),
            self.engine.step_handler.clone(),
            self.engine.clock.clone(),
        )
        .with_max_parallel_ceiling(self.engine.max_parallel_ceiling)
        .with_child_orchestrator(self.engine.child_orchestrator())
    }
}
