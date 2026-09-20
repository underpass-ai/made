use super::EmbeddedMade;
use made_app::workers::{
    CeremonyResumePreflight, InspectCeremonyResumeInput, InspectCeremonyResumeUseCase,
    RecordCeremonyHostHandoffInput, RecordCeremonyHostHandoffUseCase,
};
use made_core::entities::ceremony_events::HostHandoffRecorded;
use made_core::value_objects::AuthorizationAction;
use made_core::DomainError;

impl EmbeddedMade {
    pub async fn inspect_ceremony_resume(
        &self,
        input: InspectCeremonyResumeInput,
    ) -> Result<CeremonyResumePreflight, DomainError> {
        self.require_authorized_ceremony_action(
            AuthorizationAction::InspectCeremonyResume,
            &input.ceremony_id,
        )?;
        InspectCeremonyResumeUseCase::new(
            self.stream.clone(),
            self.execution_receipts.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn record_ceremony_host_handoff(
        &self,
        input: RecordCeremonyHostHandoffInput,
    ) -> Result<HostHandoffRecorded, DomainError> {
        self.require_authorized_ceremony_action(
            AuthorizationAction::RecordCeremonyHostHandoff,
            &input.ceremony_id,
        )?;
        let mut usecase = RecordCeremonyHostHandoffUseCase::new(
            self.stream.clone(),
            self.resolve_definition(),
            self.clock.clone(),
        );
        if let Some(authorization) = &self.authorization {
            usecase = usecase.with_reauthorization(authorization.reauthorize.clone());
        }
        Box::pin(usecase.execute(input)).await
    }
}
