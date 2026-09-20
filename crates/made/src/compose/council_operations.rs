//! Deliberation, and the three things built on it.
//!
//! A council decision, an orchestrated dispatch and a ceremony step
//! that asks a council all run the same deliberation; the only
//! difference is who asked and what is done with the answer. Built
//! together here so the one shared use case is constructed once and
//! handed out, rather than four call sites each deciding which
//! adapters a deliberation needs.

use std::sync::Arc;

use made_adapters::ceremony::DeliberatingCeremonyStepHandler;
use made_app::usecases::{DeliberateUseCase, OrchestrateUseCase, RunCouncilDecisionUseCase};
use made_core::ports::CeremonyStepHandlerPort;

use super::council_dependencies::CouncilDependencies;

/// The four handles the rest of the composition uses.
pub(super) struct CouncilOperations {
    pub(super) deliberate: Arc<DeliberateUseCase>,
    pub(super) orchestrate: Arc<OrchestrateUseCase>,
    pub(super) run_council_decision: Arc<RunCouncilDecisionUseCase>,
    pub(super) step_handler: Arc<dyn CeremonyStepHandlerPort>,
}

pub(super) fn wire(deps: CouncilDependencies) -> CouncilOperations {
    let deliberate = Arc::new(DeliberateUseCase::new(
        deps.clock.clone(),
        deps.councils.clone(),
        deps.resolver,
        deps.validators,
        deps.scoring,
        deps.repository.clone(),
        deps.messaging.clone(),
        deps.statistics.clone(),
        deps.metrics,
        "made",
    ));
    CouncilOperations {
        step_handler: Arc::new(DeliberatingCeremonyStepHandler::new(deliberate.clone())),
        orchestrate: Arc::new(OrchestrateUseCase::new(
            deliberate.clone(),
            deps.executor,
            deps.messaging,
            deps.clock,
            deps.statistics,
            "made",
        )),
        run_council_decision: Arc::new(RunCouncilDecisionUseCase::new(
            deps.contracts,
            deps.councils,
            deliberate.clone(),
            deps.repository,
        )),
        deliberate,
    }
}
