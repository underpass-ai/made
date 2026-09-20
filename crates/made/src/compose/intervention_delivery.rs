//! Putting an intervention in front of a live agent needs one ledger
//! and one roster, and all four of its verbs need the same two.
//!
//! Its own file because the rest of the operator commands share only a
//! stream, a definition resolver and a clock; these four are the one
//! family that also has a place work handed to hosts is held.

use std::sync::Arc;

use made_adapters::grpc::MadeGrpcServiceBuilder;
use made_app::services::SessionStream;
use made_app::usecases::{
    AcknowledgeCeremonyAgentInterventionUseCase, CeremonyAgentStatusService,
    GetCeremonyInterventionUseCase, ListCeremonyInterventionsUseCase,
    PullCeremonyAgentInterventionsUseCase, ResolveCeremonyDefinitionUseCase,
};
use made_core::ports::{ClockPort, HostDeliveryLedgerPort};

pub(super) fn wire<C: ClockPort + 'static>(
    builder: MadeGrpcServiceBuilder,
    definition: &Arc<ResolveCeremonyDefinitionUseCase>,
    stream: &Arc<SessionStream>,
    clock: &Arc<C>,
    deliveries: &Arc<dyn HostDeliveryLedgerPort>,
    agent_status: Arc<CeremonyAgentStatusService>,
) -> MadeGrpcServiceBuilder {
    let pull = Arc::new(PullCeremonyAgentInterventionsUseCase::new(
        stream.clone(),
        agent_status,
        deliveries.clone(),
        clock.clone(),
    ));
    let acknowledge = Arc::new(AcknowledgeCeremonyAgentInterventionUseCase::new(
        definition.clone(),
        stream.clone(),
        deliveries.clone(),
        clock.clone(),
    ));
    let get = Arc::new(GetCeremonyInterventionUseCase::new(
        stream.clone(),
        deliveries.clone(),
    ));
    let list = Arc::new(ListCeremonyInterventionsUseCase::new(
        stream.clone(),
        deliveries.clone(),
    ));
    builder
        .pull_ceremony_agent_interventions(pull)
        .acknowledge_ceremony_agent_intervention(acknowledge)
        .get_ceremony_intervention(get)
        .list_ceremony_interventions(list)
}
