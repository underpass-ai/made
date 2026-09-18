use std::sync::Arc;

use made_app::services::AutoDispatchService;
use made_app::usecases::{
    CreateCouncilUseCase, DeleteCouncilUseCase, DeliberateUseCase, GetDeliberationUseCase,
    ListCouncilsUseCase, OrchestrateUseCase, RegisterAgentUseCase, RunCouncilDecisionUseCase,
    UnregisterAgentUseCase,
};
use made_core::ports::{
    AgentFactoryPort, AgentRegistryPort, AgentResolverPort, ClockPort, ContractRegistryPort,
    CouncilRegistryPort, DeliberationRepositoryPort, ExecutorPort, MessagingPort,
    MetricsRecorderPort, ScoringPort, StatisticsPort, ValidatorPort,
};

/// Council use cases composed once for the in-process facade.
pub(crate) struct EmbeddedCouncilServices {
    pub(crate) deliberate: Arc<DeliberateUseCase>,
    pub(crate) orchestrate: Arc<OrchestrateUseCase>,
    pub(crate) get_deliberation: Arc<GetDeliberationUseCase>,
    pub(crate) create_council: Arc<CreateCouncilUseCase>,
    pub(crate) list_councils: Arc<ListCouncilsUseCase>,
    pub(crate) delete_council: Arc<DeleteCouncilUseCase>,
    pub(crate) register_agent: Arc<RegisterAgentUseCase>,
    pub(crate) unregister_agent: Arc<UnregisterAgentUseCase>,
    pub(crate) run_council_decision: Arc<RunCouncilDecisionUseCase>,
    pub(crate) auto_dispatch: Arc<AutoDispatchService>,
    pub(crate) contracts: Arc<dyn ContractRegistryPort>,
}

impl EmbeddedCouncilServices {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        clock: Arc<dyn ClockPort>,
        council_registry: Arc<dyn CouncilRegistryPort>,
        agent_registry: Arc<dyn AgentRegistryPort>,
        agent_resolver: Arc<dyn AgentResolverPort>,
        agent_factory: Arc<dyn AgentFactoryPort>,
        repository: Arc<dyn DeliberationRepositoryPort>,
        contracts: Arc<dyn ContractRegistryPort>,
        validators: Vec<Arc<dyn ValidatorPort>>,
        scoring: Arc<dyn ScoringPort>,
        executor: Arc<dyn ExecutorPort>,
        messaging: Arc<dyn MessagingPort>,
        statistics: Arc<dyn StatisticsPort>,
        metrics: Arc<dyn MetricsRecorderPort>,
    ) -> Self {
        let deliberate = Arc::new(DeliberateUseCase::new(
            clock.clone(),
            council_registry.clone(),
            agent_resolver.clone(),
            validators,
            scoring,
            repository.clone(),
            messaging.clone(),
            statistics.clone(),
            metrics,
            "made-embedded",
        ));
        let orchestrate = Arc::new(OrchestrateUseCase::new(
            deliberate.clone(),
            executor,
            messaging,
            clock.clone(),
            statistics,
            "made-embedded",
        ));
        let auto_dispatch = Arc::new(
            AutoDispatchService::new(
                deliberate.clone(),
                "Investigate the incoming trigger event.",
            )
            .expect("the built-in trigger task description is valid"),
        );
        Self {
            get_deliberation: Arc::new(GetDeliberationUseCase::new(repository.clone())),
            create_council: Arc::new(CreateCouncilUseCase::new(
                clock,
                council_registry.clone(),
                agent_resolver,
            )),
            list_councils: Arc::new(ListCouncilsUseCase::new(council_registry.clone())),
            delete_council: Arc::new(DeleteCouncilUseCase::new(council_registry.clone())),
            register_agent: Arc::new(RegisterAgentUseCase::new(
                agent_factory,
                agent_registry.clone(),
            )),
            unregister_agent: Arc::new(UnregisterAgentUseCase::new(agent_registry)),
            run_council_decision: Arc::new(RunCouncilDecisionUseCase::new(
                contracts.clone(),
                council_registry,
                deliberate.clone(),
                repository,
            )),
            deliberate,
            orchestrate,
            auto_dispatch,
            contracts,
        }
    }
}

impl std::fmt::Debug for EmbeddedCouncilServices {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EmbeddedCouncilServices")
            .finish_non_exhaustive()
    }
}
