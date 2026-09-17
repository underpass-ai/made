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
        RespondToCeremonyInterventionUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn collect_evidence(
        &self,
        input: CollectCeremonyEvidenceInput,
    ) -> Result<CeremonyInstance, DomainError> {
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
        CloseCeremonyInterventionUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }
}
