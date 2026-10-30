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
    AcknowledgeProtocolAgentInterventionUseCase, GetProtocolInterventionUseCase,
    ListProtocolInterventionsUseCase, ProtocolAgentStatusService,
    PullProtocolAgentInterventionsUseCase, ResolveProtocolDefinitionUseCase,
};
use made_core::ports::{ClockPort, HostDeliveryLedgerPort};

pub(super) fn wire<C: ClockPort + 'static>(
    builder: MadeGrpcServiceBuilder,
    definition: &Arc<ResolveProtocolDefinitionUseCase>,
    stream: &Arc<SessionStream>,
    clock: &Arc<C>,
    deliveries: &Arc<dyn HostDeliveryLedgerPort>,
    agent_status: Arc<ProtocolAgentStatusService>,
) -> MadeGrpcServiceBuilder {
    let pull = Arc::new(PullProtocolAgentInterventionsUseCase::new(
        stream.clone(),
        agent_status,
        deliveries.clone(),
        clock.clone(),
    ));
    let acknowledge = Arc::new(AcknowledgeProtocolAgentInterventionUseCase::new(
        definition.clone(),
        stream.clone(),
        deliveries.clone(),
        clock.clone(),
    ));
    let get = Arc::new(GetProtocolInterventionUseCase::new(
        stream.clone(),
        deliveries.clone(),
    ));
    let list = Arc::new(ListProtocolInterventionsUseCase::new(
        stream.clone(),
        deliveries.clone(),
    ));
    builder
        .pull_protocol_agent_interventions(pull)
        .acknowledge_protocol_agent_intervention(acknowledge)
        .get_protocol_intervention(get)
        .list_protocol_interventions(list)
}
