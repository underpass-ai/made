//! Operator commands share the service stream, definition resolver and clock.
use made_adapters::grpc::MadeGrpcServiceBuilder;
use made_adapters::noop::NoopProtocolEvidenceSource;
use made_app::services::SessionStream;
use made_app::usecases::{
    ApplyProtocolTransitionUseCase, ApproveProtocolGuardUseCase, AssertProtocolReasonUseCase,
    BindProtocolParticipantsUseCase, CloseProtocolInterventionUseCase,
    CollectProtocolEvidenceUseCase, CompleteProtocolStepUseCase, DeferProtocolGuardUseCase,
    ProtocolAgentStatusService, RequestProtocolInterventionUseCase,
    ResolveProtocolDefinitionUseCase, RespondToProtocolInterventionUseCase,
};
use made_core::ports::{ClockPort, HostDeliveryLedgerPort};
use std::sync::Arc;

use made_app::authorization::{
    ContinueAcceptedProtocolWorkUseCase, ContinueAcceptedStepClaimUseCase,
};

pub(super) fn wire<C: ClockPort + 'static>(
    builder: MadeGrpcServiceBuilder,
    definition: Arc<ResolveProtocolDefinitionUseCase>,
    stream: &Arc<SessionStream>,
    clock: &Arc<C>,
    continuation: Arc<ContinueAcceptedProtocolWorkUseCase>,
    authorize: Arc<made_app::authorization::AuthorizeOperationUseCase>,
    deliveries: &Arc<dyn HostDeliveryLedgerPort>,
    agent_status: Arc<ProtocolAgentStatusService>,
) -> MadeGrpcServiceBuilder {
    let complete_protocol_step = Arc::new(CompleteProtocolStepUseCase::new(
        definition.clone(),
        stream.clone(),
        clock.clone(),
    ));
    let apply_protocol_transition = Arc::new(ApplyProtocolTransitionUseCase::new(
        definition.clone(),
        stream.clone(),
        clock.clone(),
    ));
    let assert_protocol_reason = Arc::new(AssertProtocolReasonUseCase::new(
        definition.clone(),
        stream.clone(),
        clock.clone(),
    ));
    let approve_protocol_guard = Arc::new(ApproveProtocolGuardUseCase::new(
        definition.clone(),
        stream.clone(),
        clock.clone(),
    ));
    let defer_protocol_guard = Arc::new(DeferProtocolGuardUseCase::new(
        definition.clone(),
        stream.clone(),
        clock.clone(),
    ));
    let request_protocol_intervention = Arc::new(RequestProtocolInterventionUseCase::new(
        definition.clone(),
        stream.clone(),
        clock.clone(),
    ));
    // The ledger is composed in, not optional, because an answer that
    // names a delivery must be able to close it: leaving the route open
    // would tell an operator a question is still outstanding after it
    // has been answered.
    let respond_to_protocol_intervention = Arc::new(
        RespondToProtocolInterventionUseCase::new(
            definition.clone(),
            stream.clone(),
            clock.clone(),
        )
        .with_delivery_ledger(deliveries.clone()),
    );
    let close_protocol_intervention = Arc::new(CloseProtocolInterventionUseCase::new(
        definition.clone(),
        stream.clone(),
        clock.clone(),
    ));
    // No evidence source ships with the server, so this answers
    // NOT_FOUND until an operator wires one. Failing plainly beats a
    // missing method or an invented answer.
    let collect_protocol_evidence = Arc::new(CollectProtocolEvidenceUseCase::new(
        definition.clone(),
        stream.clone(),
        Arc::new(NoopProtocolEvidenceSource::new()),
        clock.clone(),
    ));
    let bind_protocol_participants = Arc::new(BindProtocolParticipantsUseCase::new(
        definition.clone(),
        stream.clone(),
        clock.clone(),
    ));
    let builder = super::intervention_delivery::wire(
        builder,
        &definition,
        stream,
        clock,
        deliveries,
        agent_status,
    );
    builder
        .renew_protocol_step_lease(Arc::new(
            made_app::workers::RenewProtocolStepLeaseUseCase::new(
                stream.clone(),
                definition,
                clock.clone(),
            )
            .with_reauthorization(authorize),
        ))
        .continue_accepted_step_claim(Arc::new(ContinueAcceptedStepClaimUseCase::new(
            stream.clone(),
            continuation,
            clock.clone(),
        )))
        .complete_protocol_step(complete_protocol_step)
        .apply_protocol_transition(apply_protocol_transition)
        .assert_protocol_reason(assert_protocol_reason)
        .approve_protocol_guard(approve_protocol_guard)
        .defer_protocol_guard(defer_protocol_guard)
        .request_protocol_intervention(request_protocol_intervention)
        .respond_to_protocol_intervention(respond_to_protocol_intervention)
        .close_protocol_intervention(close_protocol_intervention)
        .collect_protocol_evidence(collect_protocol_evidence)
        .bind_protocol_participants(bind_protocol_participants)
}

/// Catch up persisted child work on startup without requiring a broker notification.
pub(super) async fn recover_to_head(
    recovery: &made_app::usecases::RecoverProtocolChildrenUseCase,
) -> Result<(), made_core::DomainError> {
    loop {
        let limit = made_core::value_objects::ProtocolEventPageLimit::DEFAULT;
        let round = recovery.execute(limit).await?;
        if round.busy || round.failed > 0 || round.acknowledged() < limit.value() {
            break;
        }
    }
    Ok(())
}
