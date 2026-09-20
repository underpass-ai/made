//! Operator commands share the service stream, definition resolver and clock.
use made_adapters::grpc::MadeGrpcServiceBuilder;
use made_adapters::noop::NoopCeremonyEvidenceSource;
use made_app::services::SessionStream;
use made_app::usecases::{
    ApplyCeremonyTransitionUseCase, ApproveCeremonyGuardUseCase, AssertCeremonyReasonUseCase,
    BindCeremonyParticipantsUseCase, CeremonyAgentStatusService, CloseCeremonyInterventionUseCase,
    CollectCeremonyEvidenceUseCase, CompleteCeremonyStepUseCase, DeferCeremonyGuardUseCase,
    RequestCeremonyInterventionUseCase, ResolveCeremonyDefinitionUseCase,
    RespondToCeremonyInterventionUseCase,
};
use made_core::ports::{ClockPort, HostDeliveryLedgerPort};
use std::sync::Arc;

use made_app::authorization::{
    ContinueAcceptedCeremonyWorkUseCase, ContinueAcceptedStepClaimUseCase,
};

pub(super) fn wire<C: ClockPort + 'static>(
    builder: MadeGrpcServiceBuilder,
    definition: Arc<ResolveCeremonyDefinitionUseCase>,
    stream: &Arc<SessionStream>,
    clock: &Arc<C>,
    continuation: Arc<ContinueAcceptedCeremonyWorkUseCase>,
    authorize: Arc<made_app::authorization::AuthorizeOperationUseCase>,
    deliveries: &Arc<dyn HostDeliveryLedgerPort>,
    agent_status: Arc<CeremonyAgentStatusService>,
) -> MadeGrpcServiceBuilder {
    let complete_ceremony_step = Arc::new(CompleteCeremonyStepUseCase::new(
        definition.clone(),
        stream.clone(),
        clock.clone(),
    ));
    let apply_ceremony_transition = Arc::new(ApplyCeremonyTransitionUseCase::new(
        definition.clone(),
        stream.clone(),
        clock.clone(),
    ));
    let assert_ceremony_reason = Arc::new(AssertCeremonyReasonUseCase::new(
        definition.clone(),
        stream.clone(),
        clock.clone(),
    ));
    let approve_ceremony_guard = Arc::new(ApproveCeremonyGuardUseCase::new(
        definition.clone(),
        stream.clone(),
        clock.clone(),
    ));
    let defer_ceremony_guard = Arc::new(DeferCeremonyGuardUseCase::new(
        definition.clone(),
        stream.clone(),
        clock.clone(),
    ));
    let request_ceremony_intervention = Arc::new(RequestCeremonyInterventionUseCase::new(
        definition.clone(),
        stream.clone(),
        clock.clone(),
    ));
    // The ledger is composed in, not optional, because an answer that
    // names a delivery must be able to close it: leaving the route open
    // would tell an operator a question is still outstanding after it
    // has been answered.
    let respond_to_ceremony_intervention = Arc::new(
        RespondToCeremonyInterventionUseCase::new(
            definition.clone(),
            stream.clone(),
            clock.clone(),
        )
        .with_delivery_ledger(deliveries.clone()),
    );
    let close_ceremony_intervention = Arc::new(CloseCeremonyInterventionUseCase::new(
        definition.clone(),
        stream.clone(),
        clock.clone(),
    ));
    // No evidence source ships with the server, so this answers
    // NOT_FOUND until an operator wires one. Failing plainly beats a
    // missing method or an invented answer.
    let collect_ceremony_evidence = Arc::new(CollectCeremonyEvidenceUseCase::new(
        definition.clone(),
        stream.clone(),
        Arc::new(NoopCeremonyEvidenceSource::new()),
        clock.clone(),
    ));
    let bind_ceremony_participants = Arc::new(BindCeremonyParticipantsUseCase::new(
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
        .renew_ceremony_step_lease(Arc::new(
            made_app::workers::RenewCeremonyStepLeaseUseCase::new(
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
        .complete_ceremony_step(complete_ceremony_step)
        .apply_ceremony_transition(apply_ceremony_transition)
        .assert_ceremony_reason(assert_ceremony_reason)
        .approve_ceremony_guard(approve_ceremony_guard)
        .defer_ceremony_guard(defer_ceremony_guard)
        .request_ceremony_intervention(request_ceremony_intervention)
        .respond_to_ceremony_intervention(respond_to_ceremony_intervention)
        .close_ceremony_intervention(close_ceremony_intervention)
        .collect_ceremony_evidence(collect_ceremony_evidence)
        .bind_ceremony_participants(bind_ceremony_participants)
}

/// Catch up persisted child work on startup without requiring a broker notification.
pub(super) async fn recover_to_head(
    recovery: &made_app::usecases::RecoverCeremonyChildrenUseCase,
) -> Result<(), made_core::DomainError> {
    loop {
        let limit = made_core::value_objects::CeremonyEventPageLimit::DEFAULT;
        let round = recovery.execute(limit).await?;
        if round.busy || round.failed > 0 || round.acknowledged() < limit.value() {
            break;
        }
    }
    Ok(())
}
