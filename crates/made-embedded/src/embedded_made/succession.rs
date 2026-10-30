use super::EmbeddedMade;
use made_app::usecases::{
    PlanProtocolSuccessorInput, PlanProtocolSuccessorUseCase, ProtocolSuccessorOutcome,
    ProtocolSuccessorPlanView, StartProtocolSuccessorInput, StartProtocolSuccessorUseCase,
};
use made_core::value_objects::AuthorizationAction;
use made_core::DomainError;
use std::sync::Arc;

impl EmbeddedMade {
    /// What handing this protocol to a successor would involve.
    pub async fn plan_successor(
        &self,
        input: PlanProtocolSuccessorInput,
    ) -> Result<ProtocolSuccessorPlanView, DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::PlanProtocolSuccessor,
            &input.instance_id,
        )?;
        PlanProtocolSuccessorUseCase::new(
            self.resolve_definition(),
            self.publications.clone(),
            self.stream.clone(),
            Arc::new(made_app::workers::InspectProtocolResumeUseCase::new(
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
    /// Authorized twice and separately: on the protocol handing off,
    /// and on the definition the successor would run. Planning a
    /// successor does not grant the right to start one, and the right
    /// to end this protocol is not the right to open a session under
    /// somebody else's definition.
    pub async fn start_successor(
        &self,
        input: StartProtocolSuccessorInput,
    ) -> Result<ProtocolSuccessorOutcome, DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::StartProtocolSuccessor,
            &input.instance_id,
        )?;
        let mut usecase = StartProtocolSuccessorUseCase::new(
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
