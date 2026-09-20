//! Deliberation and the council verbs built on it.
//!
//! One family: every one of these is the same deliberation use case
//! seen from a different caller, and the step handler is that use case
//! standing in for a host. Wiring them together keeps the shared
//! instance shared rather than rebuilt per caller.

use std::sync::Arc;

use made_adapters::ceremony::DeliberatingCeremonyStepHandler;
use made_app::usecases::{DeliberateUseCase, OrchestrateUseCase, RunCouncilDecisionUseCase};
use made_core::ports::CeremonyStepHandlerPort;

use super::council_ports::CouncilPorts;

/// What it produces, all sharing one deliberation use case.
pub(super) struct CouncilOperations {
    pub(super) deliberate: Arc<DeliberateUseCase>,
    pub(super) ceremony_step_handler: Arc<dyn CeremonyStepHandlerPort>,
    pub(super) orchestrate: Arc<OrchestrateUseCase>,
    pub(super) run_council_decision: Arc<RunCouncilDecisionUseCase>,
}

pub(super) fn wire(ports: CouncilPorts) -> CouncilOperations {
    let deliberate = Arc::new(DeliberateUseCase::new(
        ports.clock.clone(),
        ports.council_registry.clone(),
        ports.agent_resolver.clone(),
        ports.validators,
        ports.scoring,
        ports.repository.clone(),
        ports.messaging.clone(),
        ports.statistics.clone(),
        ports.metrics.clone(),
        "made",
    ));

    let ceremony_step_handler: Arc<dyn CeremonyStepHandlerPort> =
        Arc::new(DeliberatingCeremonyStepHandler::new(deliberate.clone()));

    let orchestrate = Arc::new(OrchestrateUseCase::new(
        deliberate.clone(),
        ports.executor,
        ports.messaging.clone(),
        ports.clock.clone(),
        ports.statistics.clone(),
        "made",
    ));

    let run_council_decision = Arc::new(RunCouncilDecisionUseCase::new(
        ports.contract_registry.clone(),
        ports.council_registry.clone(),
        deliberate.clone(),
        ports.repository.clone(),
    ));

    CouncilOperations {
        deliberate,
        ceremony_step_handler,
        orchestrate,
        run_council_decision,
    }
}
