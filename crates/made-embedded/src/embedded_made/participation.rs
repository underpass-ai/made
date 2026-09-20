use made_app::usecases::{
    ApproveCeremonyGuardInput, ApproveCeremonyGuardUseCase, AssertCeremonyReasonInput,
    AssertCeremonyReasonUseCase, CloseCeremonyInterventionInput, CloseCeremonyInterventionUseCase,
    CollectCeremonyEvidenceInput, CollectCeremonyEvidenceUseCase, DeferCeremonyGuardInput,
    DeferCeremonyGuardUseCase, RequestCeremonyInterventionInput,
    RequestCeremonyInterventionUseCase, RespondToCeremonyInterventionInput,
    RespondToCeremonyInterventionUseCase,
};
use made_core::entities::CeremonyInstance;
use made_core::error::DomainError;
use made_core::value_objects::AuthorizationAction;

use super::EmbeddedMade;

impl EmbeddedMade {
    /// Say why one thing this session produced led to another.
    ///
    /// In-process only for now, and deliberately: a host embedding the
    /// engine can record its reasoning today without a wire format
    /// being settled for it.
    pub async fn assert_reason(
        &self,
        input: AssertCeremonyReasonInput,
    ) -> Result<CeremonyInstance, DomainError> {
        self.require_authorized_ceremony_action(
            AuthorizationAction::AssertCeremonyReason,
            input.instance_id(),
        )?;
        AssertCeremonyReasonUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn approve_guard(
        &self,
        input: ApproveCeremonyGuardInput,
    ) -> Result<CeremonyInstance, DomainError> {
        self.require_authorized_ceremony_action(
            AuthorizationAction::ApproveCeremonyGuard,
            input.instance_id(),
        )?;
        ApproveCeremonyGuardUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn defer_guard(
        &self,
        input: DeferCeremonyGuardInput,
    ) -> Result<CeremonyInstance, DomainError> {
        self.require_authorized_ceremony_action(
            AuthorizationAction::DeferCeremonyGuard,
            input.instance_id(),
        )?;
        DeferCeremonyGuardUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn request_intervention(
        &self,
        input: RequestCeremonyInterventionInput,
    ) -> Result<CeremonyInstance, DomainError> {
        self.require_authorized_ceremony_action(
            AuthorizationAction::RequestCeremonyIntervention,
            input.instance_id(),
        )?;
        RequestCeremonyInterventionUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn respond_to_intervention(
        &self,
        input: RespondToCeremonyInterventionInput,
    ) -> Result<CeremonyInstance, DomainError> {
        self.require_authorized_ceremony_action(
            AuthorizationAction::RespondToCeremonyIntervention,
            input.instance_id(),
        )?;
        // The ledger is composed in so an answer that names a delivery
        // can close it. Without it, a route stays open after the
        // question it carried has been answered, and an operator is
        // told something is outstanding that is not.
        RespondToCeremonyInterventionUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .with_delivery_ledger(self.host_delivery_ledger().clone())
        .execute(input)
        .await
    }

    pub async fn collect_evidence(
        &self,
        input: CollectCeremonyEvidenceInput,
    ) -> Result<CeremonyInstance, DomainError> {
        self.require_authorized_ceremony_action(
            AuthorizationAction::CollectCeremonyEvidence,
            input.instance_id(),
        )?;
        CollectCeremonyEvidenceUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.evidence_source.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn close_intervention(
        &self,
        input: CloseCeremonyInterventionInput,
    ) -> Result<CeremonyInstance, DomainError> {
        self.require_authorized_ceremony_action(
            AuthorizationAction::CloseCeremonyIntervention,
            input.instance_id(),
        )?;
        CloseCeremonyInterventionUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }
}
