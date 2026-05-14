//! In-process gRPC fixture for Epic 9 (and any later RPC slice that
//! wants to drive the real choreographer over a Tonic channel).
//!
//! The fixture wires the full composition graph in-memory — NATS
//! disabled, Postgres disabled, executor=noop — then mounts the
//! resulting `ChoreographerGrpcService` on a `tokio::net::TcpListener`
//! bound to `127.0.0.1:0`. Tests get back a `tonic::transport::Channel`
//! plus handles to the in-memory contract / council / agent
//! registries, so they can seed state through the registry ports
//! and assert through the RPC surface in the same test.
//!
//! Why a manual wire instead of `choreo::compose::compose`?
//! `compose` reads from process env and would force every test that
//! uses this fixture to serialize through a shared lock — concurrent
//! `cargo test --workspace` would either deadlock or fight over env
//! vars. The fixture instead constructs the use-case graph with
//! explicit deps so each test gets a fresh, isolated registry set.

use std::sync::Arc;
use std::time::Duration;

use choreo_adapters::agents::DispatchingAgentFactory;
use choreo_adapters::clock::SystemClock;
use choreo_adapters::grpc::ChoreographerGrpcService;
use choreo_adapters::memory::{
    InMemoryAgentRegistry, InMemoryContractRegistry, InMemoryCouncilRegistry,
    InMemoryDeliberationRepository, InMemoryStatistics,
};
use choreo_adapters::noop::{NoopExecutor, NoopMessaging};
use choreo_adapters::scoring::UniformScoring;
use choreo_adapters::validators::{
    AllowedStringValuesValidator, ContentNonEmptyValidator, JsonObjectOutputValidator,
    JsonSchemaValidator, RequiredFieldsValidator,
};
use choreo_app::services::AutoDispatchService;
use choreo_app::usecases::{
    CreateCouncilUseCase, DeleteCouncilUseCase, DeliberateUseCase, GetDeliberationUseCase,
    ListCouncilsUseCase, OrchestrateUseCase, RegisterAgentUseCase, RunCouncilDecisionUseCase,
    UnregisterAgentUseCase,
};
use choreo_core::ports::{
    AgentRegistryPort, AgentResolverPort, ContractRegistryPort, CouncilRegistryPort, ValidatorPort,
};
use tokio::sync::oneshot;
use tonic::transport::{Channel, Endpoint, Server};

/// Handles a test needs to drive the in-process choreographer:
/// the gRPC channel for issuing RPCs and the registries for seeding
/// state without going through CRUD calls.
pub struct GrpcFixture {
    pub channel: Channel,
    pub contracts: Arc<dyn ContractRegistryPort>,
    pub councils: Arc<dyn CouncilRegistryPort>,
    pub agents: Arc<dyn AgentRegistryPort>,
    pub agent_resolver: Arc<dyn AgentResolverPort>,
    /// Held for the lifetime of the fixture; drop kills the server.
    _shutdown: oneshot::Sender<()>,
}

impl GrpcFixture {
    /// Wire and start a fresh fixture. Returns once the server is
    /// accepting connections.
    #[allow(clippy::too_many_lines)] // wiring graph mirrors `compose::compose`; splitting fragments the dep order
    pub async fn start() -> Self {
        let clock = Arc::new(SystemClock::new());
        let validators: Vec<Arc<dyn ValidatorPort>> = vec![
            Arc::new(ContentNonEmptyValidator::new()),
            Arc::new(JsonObjectOutputValidator::new()),
            Arc::new(RequiredFieldsValidator::new()),
            Arc::new(AllowedStringValuesValidator::new()),
            Arc::new(JsonSchemaValidator::new()),
        ];
        let scoring = Arc::new(UniformScoring::new());
        let executor = Arc::new(NoopExecutor::new());
        let messaging = Arc::new(NoopMessaging::new());
        let statistics = Arc::new(InMemoryStatistics::new());
        let repository = Arc::new(InMemoryDeliberationRepository::new());
        let council_registry: Arc<dyn CouncilRegistryPort> =
            Arc::new(InMemoryCouncilRegistry::new());
        let contract_registry: Arc<dyn ContractRegistryPort> =
            Arc::new(InMemoryContractRegistry::new());
        let agent_registry = Arc::new(InMemoryAgentRegistry::new());
        let agent_resolver: Arc<dyn AgentResolverPort> = agent_registry.clone();
        // `new()` yields a factory that supports only the always-on
        // `noop` kind — exactly what these tests need.
        let agent_factory = Arc::new(DispatchingAgentFactory::new());

        let deliberate = Arc::new(DeliberateUseCase::new(
            clock.clone(),
            council_registry.clone(),
            agent_resolver.clone(),
            validators,
            scoring,
            repository.clone(),
            messaging.clone(),
            statistics.clone(),
            "choreographer-tests",
        ));
        let orchestrate = Arc::new(OrchestrateUseCase::new(
            deliberate.clone(),
            executor,
            messaging.clone(),
            clock.clone(),
            statistics.clone(),
            "choreographer-tests",
        ));
        let run_council_decision = Arc::new(RunCouncilDecisionUseCase::new(
            contract_registry.clone(),
            council_registry.clone(),
            deliberate.clone(),
            repository.clone(),
        ));
        let create_council = Arc::new(CreateCouncilUseCase::new(
            clock.clone(),
            council_registry.clone(),
            agent_resolver.clone(),
        ));
        let delete_council = Arc::new(DeleteCouncilUseCase::new(council_registry.clone()));
        let list_councils = Arc::new(ListCouncilsUseCase::new(council_registry.clone()));
        let get_deliberation = Arc::new(GetDeliberationUseCase::new(repository.clone()));
        let register_agent = Arc::new(RegisterAgentUseCase::new(
            agent_factory.clone(),
            agent_registry.clone(),
        ));
        let unregister_agent = Arc::new(UnregisterAgentUseCase::new(agent_registry.clone()));
        let auto_dispatch = Arc::new(
            AutoDispatchService::new(
                deliberate.clone(),
                "Investigate the incoming trigger event.",
            )
            .expect("auto-dispatch wiring should never fail"),
        );

        let svc = ChoreographerGrpcService::builder()
            .deliberate(deliberate)
            .orchestrate(orchestrate)
            .create_council(create_council)
            .delete_council(delete_council)
            .list_councils(list_councils)
            .get_deliberation(get_deliberation)
            .register_agent(register_agent)
            .unregister_agent(unregister_agent)
            .run_council_decision(run_council_decision)
            .contract_registry(contract_registry.clone())
            .auto_dispatch(auto_dispatch)
            .statistics(statistics.clone())
            .service_version("choreographer-tests")
            .build()
            .expect("grpc service wiring should succeed");

        // Bind on an ephemeral port so concurrent fixtures don't clash.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("ephemeral bind should succeed");
        let addr = listener.local_addr().expect("local_addr");
        let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        tokio::spawn(async move {
            let _ = Server::builder()
                .add_service(svc.into_server())
                .serve_with_incoming_shutdown(incoming, async move {
                    let _ = shutdown_rx.await;
                })
                .await;
        });

        // Build a Channel pointed at the spawned server. Endpoint is
        // not resolved synchronously by tonic; connect_lazy() defers
        // the first connection to the first RPC call.
        let channel = Endpoint::from_shared(format!("http://{addr}"))
            .expect("endpoint URL")
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .connect_lazy();

        GrpcFixture {
            channel,
            contracts: contract_registry,
            councils: council_registry,
            agents: agent_registry,
            agent_resolver,
            _shutdown: shutdown_tx,
        }
    }
}
