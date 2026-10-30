use made_app::usecases::{
    ApproveProtocolGuardInput, ApproveProtocolGuardUseCase, AssertProtocolReasonInput,
    AssertProtocolReasonUseCase, CloseProtocolInterventionInput, CloseProtocolInterventionUseCase,
    CollectProtocolEvidenceInput, CollectProtocolEvidenceUseCase, DeferProtocolGuardInput,
    DeferProtocolGuardUseCase, RequestProtocolInterventionInput,
    RequestProtocolInterventionUseCase, RespondToProtocolInterventionInput,
    RespondToProtocolInterventionUseCase,
};
use made_core::entities::ProtocolInstance;
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
        input: AssertProtocolReasonInput,
    ) -> Result<ProtocolInstance, DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::AssertProtocolReason,
            input.instance_id(),
        )?;
        AssertProtocolReasonUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn approve_guard(
        &self,
        input: ApproveProtocolGuardInput,
    ) -> Result<ProtocolInstance, DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::ApproveProtocolGuard,
            input.instance_id(),
        )?;
        ApproveProtocolGuardUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn defer_guard(
        &self,
        input: DeferProtocolGuardInput,
    ) -> Result<ProtocolInstance, DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::DeferProtocolGuard,
            input.instance_id(),
        )?;
        DeferProtocolGuardUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn request_intervention(
        &self,
        input: RequestProtocolInterventionInput,
    ) -> Result<ProtocolInstance, DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::RequestProtocolIntervention,
            input.instance_id(),
        )?;
        RequestProtocolInterventionUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn respond_to_intervention(
        &self,
        input: RespondToProtocolInterventionInput,
    ) -> Result<ProtocolInstance, DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::RespondToProtocolIntervention,
            input.instance_id(),
        )?;
        // The ledger is composed in so an answer that names a delivery
        // can close it. Without it, a route stays open after the
        // question it carried has been answered, and an operator is
        // told something is outstanding that is not.
        RespondToProtocolInterventionUseCase::new(
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
        input: CollectProtocolEvidenceInput,
    ) -> Result<ProtocolInstance, DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::CollectProtocolEvidence,
            input.instance_id(),
        )?;
        CollectProtocolEvidenceUseCase::new(
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
        input: CloseProtocolInterventionInput,
    ) -> Result<ProtocolInstance, DomainError> {
        self.require_authorized_protocol_action(
            AuthorizationAction::CloseProtocolIntervention,
            input.instance_id(),
        )?;
        CloseProtocolInterventionUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }
}
