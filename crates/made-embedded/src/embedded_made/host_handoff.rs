use super::EmbeddedMade;
use made_app::workers::{
    InspectProtocolResumeInput, InspectProtocolResumeUseCase, ProtocolResumePreflight,
    RecordProtocolHostHandoffInput, RecordProtocolHostHandoffUseCase,
};
use made_core::entities::protocol_events::HostHandoffRecorded;
use made_core::value_objects::AuthorizationAction;
use made_core::DomainError;

impl EmbeddedMade {
    pub async fn inspect_protocol_resume(
        &self,
        input: InspectProtocolResumeInput,
    ) -> Result<ProtocolResumePreflight, DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::InspectProtocolResume,
            &input.protocol_id,
        )?;
        InspectProtocolResumeUseCase::new(
            self.stream.clone(),
            self.execution_receipts.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn record_protocol_host_handoff(
        &self,
        input: RecordProtocolHostHandoffInput,
    ) -> Result<HostHandoffRecorded, DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::RecordProtocolHostHandoff,
            &input.protocol_id,
        )?;
        let mut usecase = RecordProtocolHostHandoffUseCase::new(
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
