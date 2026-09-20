use super::EmbeddedMade;
use made_app::authorization::AuthorizationGateOutcome;
use made_app::usecases::{
    CeremonySuccessorOutcome, CeremonySuccessorPlanView, PlanCeremonySuccessorInput,
    PlanCeremonySuccessorUseCase, StartCeremonySuccessorInput, StartCeremonySuccessorUseCase,
};
use made_core::value_objects::{
    AuthorizationAction, AuthorizationRequest, AuthorizationRequestId, AuthorizationScope,
    AuthorizationTargetDigest, CeremonyName, CeremonyVersion,
};
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
        self.require_successor_definition_admitted(
            &input.definition_name,
            &input.definition_version,
        )
        .await?;
        StartCeremonySuccessorUseCase::new(
            self.resolve_definition(),
            self.publications.clone(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    /// Whether this caller may open a session under the successor's
    /// definition, asked as its own question.
    ///
    /// The operation in flight is scoped to the ceremony handing off,
    /// so it cannot also carry a definition scope: this is a second
    /// admission, decided against the same policy and the same
    /// principal, for the definition the successor would run. Without
    /// it, ending one ceremony would silently confer the right to start
    /// another under somebody else's definition.
    async fn require_successor_definition_admitted(
        &self,
        name: &CeremonyName,
        version: &CeremonyVersion,
    ) -> Result<(), DomainError> {
        let Some(authorization) = &self.authorization else {
            return Ok(());
        };
        let operation = made_app::services::AuthorizationOperationScope::current().ok_or(
            DomainError::InvariantViolated {
                reason: "protected embedded facade requires an authorized operation context",
            },
        )?;
        let target = format!("{}@{}", name.as_str(), version.as_str());
        let request = AuthorizationRequest::new(
            AuthorizationRequestId::new(format!("successor-definition:{target}"))?,
            operation.principal().clone(),
            AuthorizationAction::StartPublishedCeremony,
            AuthorizationScope::Definition {
                name: name.clone(),
                version: Some(version.clone()),
            },
            AuthorizationTargetDigest::for_bytes(target.as_bytes()),
        );
        match authorization.reauthorize.execute(request).await? {
            AuthorizationGateOutcome::Allowed { .. } => Ok(()),
            AuthorizationGateOutcome::Denied { .. } | AuthorizationGateOutcome::Expired { .. } => {
                Err(DomainError::InvariantViolated {
                    reason: "current authorization does not admit the successor's definition",
                })
            }
        }
    }
}
