use super::EmbeddedMade;
use made_app::usecases::{
    CeremonySuccessorOutcome, CeremonySuccessorPlanView, PlanCeremonySuccessorInput,
    PlanCeremonySuccessorUseCase, StartCeremonySuccessorInput, StartCeremonySuccessorUseCase,
};
use made_core::value_objects::AuthorizationAction;
use made_core::DomainError;
use std::sync::Arc;

impl EmbeddedMade {
    /// What handing this ceremony to a successor would involve.
    pub async fn plan_successor(
        &self,
        input: PlanCeremonySuccessorInput,
    ) -> Result<CeremonySuccessorPlanView, DomainError> {
        self.require_authorized_ceremony_action(
            AuthorizationAction::PlanCeremonySuccessor,
            &input.instance_id,
        )?;
        PlanCeremonySuccessorUseCase::new(
            self.resolve_definition(),
            self.publications.clone(),
            self.stream.clone(),
            Arc::new(made_app::workers::InspectCeremonyResumeUseCase::new(
                self.stream.clone(),
                self.execution_receipts.clone(),
                self.clock.clone(),
            )),
        )
        .execute(input)
        .await
    }

    /// Seal the handoff, then open the successor it names.
    ///
    /// Authorized twice and separately: on the ceremony handing off,
    /// and on the definition the successor would run. Planning a
    /// successor does not grant the right to start one, and the right
    /// to end this ceremony is not the right to open a session under
    /// somebody else's definition.
    pub async fn start_successor(
        &self,
        input: StartCeremonySuccessorInput,
    ) -> Result<CeremonySuccessorOutcome, DomainError> {
        self.require_authorized_ceremony_action(
            AuthorizationAction::StartCeremonySuccessor,
            &input.instance_id,
        )?;
        let mut usecase = StartCeremonySuccessorUseCase::new(
            self.resolve_definition(),
            self.publications.clone(),
            self.stream.clone(),
            self.clock.clone(),
        );
        if let Some(authorization) = &self.authorization {
            usecase = usecase.with_reauthorization(authorization.reauthorize.clone());
        }
        Box::pin(usecase.execute(input)).await
    }
}
