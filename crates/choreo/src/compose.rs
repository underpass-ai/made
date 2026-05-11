//! Wire every adapter and use case into a runnable [`Application`].

use std::sync::Arc;

use choreo_adapters::agents::DispatchingAgentFactory;
use choreo_adapters::clock::SystemClock;
use choreo_adapters::config::EnvConfiguration;
use choreo_adapters::memory::{
    InMemoryAgentRegistry, InMemoryCouncilRegistry, InMemoryDeliberationRepository,
    InMemoryStatistics,
};
use choreo_adapters::nats::{NatsConfig, NatsMessaging, NatsTriggerSubscriber};
use choreo_adapters::noop::{NoopExecutor, NoopMessaging};
use choreo_adapters::postgres::{
    PostgresAgentRegistry, PostgresConfig, PostgresCouncilRegistry, PostgresDeliberationRepository,
    PostgresPool, PostgresPoolError, PostgresStatistics,
};
use choreo_adapters::runtime::{
    ExecutorBackendConfig, RuntimeExecutor, RuntimeExecutorConnectError,
};
use choreo_adapters::scoring::UniformScoring;
use choreo_adapters::validators::{
    AllowedStringValuesValidator, ContentNonEmptyValidator, JsonObjectOutputValidator,
    RequiredFieldsValidator,
};
use choreo_app::services::AutoDispatchService;
use choreo_app::usecases::{
    CreateCouncilUseCase, DeleteCouncilUseCase, DeliberateUseCase, GetDeliberationUseCase,
    ListCouncilsUseCase, OrchestrateUseCase, RegisterAgentUseCase, UnregisterAgentUseCase,
};
use choreo_core::error::DomainError;
use choreo_core::ports::{
    AgentFactoryPort, AgentRegistryPort, AgentResolverPort, ConfigurationPort, CouncilRegistryPort,
    DeliberationRepositoryPort, ExecutorPort, MessagingPort, ScoringPort, ServiceConfig,
    StatisticsPort, ValidatorPort,
};
use thiserror::Error;
use tracing::info;

use crate::seeding::SeedingError;

/// Aggregate of every handle the composition root produces.
pub struct Application {
    pub service_config: ServiceConfig,
    pub agent_registry: Arc<dyn AgentRegistryPort>,
    pub agent_resolver: Arc<dyn AgentResolverPort>,
    pub council_registry: Arc<dyn CouncilRegistryPort>,
    pub repository: Arc<dyn DeliberationRepositoryPort>,
    pub grpc_service: choreo_adapters::grpc::ChoreographerGrpcService,
    pub nats_subscriber: Option<NatsTriggerSubscriber>,
    pub health_state: crate::health::HealthState,
}

impl std::fmt::Debug for Application {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Application")
            .field("service_config", &self.service_config)
            .field("nats_subscriber_enabled", &self.nats_subscriber.is_some())
            .finish()
    }
}

/// Errors produced while composing the application.
#[derive(Debug, Error)]
pub enum ComposeError {
    #[error("domain error during wiring: {0}")]
    Domain(#[from] DomainError),

    #[error("nats connection failed: {0}")]
    NatsConnect(#[source] async_nats::ConnectError),

    #[error("postgres setup failed: {0}")]
    Postgres(#[from] PostgresPoolError),

    #[error("seeding failed: {0}")]
    Seeding(#[from] SeedingError),

    #[error("runtime executor setup failed: {0}")]
    RuntimeExecutor(#[from] RuntimeExecutorConnectError),
}

/// Wire the full application.
///
/// - Reads [`ServiceConfig`] from the environment.
/// - Builds the in-memory registries plus the configured execution
///   backend. `noop` remains the default; richer executors are
///   selected explicitly by deployment configuration.
/// - When `nats_enabled`, connects to NATS and wires both the
///   outbound `NatsMessaging` and the inbound `NatsTriggerSubscriber`.
///   Otherwise uses [`NoopMessaging`].
/// - Optionally seeds demo councils if `CHOREO_SEED_SPECIALTIES` is
///   set, so an empty deployment is immediately exercisable against
///   the AsyncAPI / gRPC contract.
#[allow(clippy::too_many_lines)]
pub async fn compose() -> Result<Application, ComposeError> {
    let service_config = EnvConfiguration::new().load().await?;

    let clock = Arc::new(SystemClock::new());
    let validators: Vec<Arc<dyn ValidatorPort>> = vec![
        Arc::new(ContentNonEmptyValidator::new()),
        Arc::new(JsonObjectOutputValidator::new()),
        Arc::new(RequiredFieldsValidator::new()),
        Arc::new(AllowedStringValuesValidator::new()),
    ];
    let scoring: Arc<dyn ScoringPort> = Arc::new(UniformScoring::new());
    let executor = wire_executor().await?;
    let dispatching_factory = DispatchingAgentFactory::from_env()?;
    let supported_agent_kinds = dispatching_factory.supported_kinds().join(",");
    let agent_factory: Arc<dyn AgentFactoryPort> = Arc::new(dispatching_factory);

    // Pick the persistent backings together so the three registries
    // and the deliberation repository always live on the same pool
    // (or all in-memory). Running one Postgres and two in-memory
    // would split the source of truth across replicas.
    let Persistence {
        repository,
        council_registry,
        agent_registry,
        agent_resolver,
        statistics,
    } = wire_persistence(&service_config, agent_factory.clone()).await?;

    let MessagingWiring {
        port: messaging,
        subscriber_factory: nats_subscriber_factory,
        nats_client,
    } = wire_messaging(&service_config).await?;

    let deliberate = Arc::new(DeliberateUseCase::new(
        clock.clone(),
        council_registry.clone(),
        agent_resolver.clone(),
        validators,
        scoring,
        repository.clone(),
        messaging.clone(),
        statistics.clone(),
        "choreographer",
    ));

    let orchestrate = Arc::new(OrchestrateUseCase::new(
        deliberate.clone(),
        executor,
        messaging.clone(),
        clock.clone(),
        statistics.clone(),
        "choreographer",
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
        agent_factory,
        agent_registry.clone(),
    ));
    let unregister_agent = Arc::new(UnregisterAgentUseCase::new(agent_registry.clone()));

    let auto_dispatch = Arc::new(AutoDispatchService::new(
        deliberate.clone(),
        "Investigate the incoming trigger event.",
    )?);

    // Seeding — keeps the service exercisable on a fresh boot.
    crate::seeding::apply_env_seeding(
        clock.as_ref(),
        agent_registry.as_ref(),
        council_registry.as_ref(),
    )
    .await?;

    // Now that the auto-dispatch service exists, the subscriber
    // factory can finish wiring.
    let nats_subscriber = nats_subscriber_factory.map(|factory| factory(auto_dispatch.clone()));

    let grpc_service = choreo_adapters::grpc::ChoreographerGrpcService::builder()
        .deliberate(deliberate)
        .orchestrate(orchestrate)
        .create_council(create_council)
        .delete_council(delete_council)
        .list_councils(list_councils)
        .get_deliberation(get_deliberation)
        .register_agent(register_agent)
        .unregister_agent(unregister_agent)
        .auto_dispatch(auto_dispatch)
        .statistics(statistics.clone())
        .service_version(env!("CARGO_PKG_VERSION"))
        .build()?;

    let health_state =
        crate::health::HealthState::new(nats_client, statistics.clone(), env!("CARGO_PKG_VERSION"));

    info!(
        grpc_port = service_config.grpc_port,
        http_port = service_config.http_port,
        nats_enabled = service_config.nats_enabled,
        executor_backend = executor_backend_name(),
        agent_kinds = supported_agent_kinds.as_str(),
        trigger_subject = service_config.trigger_subject.as_str(),
        "choreographer wired"
    );

    Ok(Application {
        service_config,
        agent_registry,
        agent_resolver,
        council_registry,
        repository,
        grpc_service,
        nats_subscriber,
        health_state,
    })
}

async fn wire_executor() -> Result<Arc<dyn ExecutorPort>, ComposeError> {
    let executor: Arc<dyn ExecutorPort> = match ExecutorBackendConfig::from_env()? {
        ExecutorBackendConfig::Noop => Arc::new(NoopExecutor::new()),
        ExecutorBackendConfig::Runtime(config) => Arc::new(RuntimeExecutor::connect(config).await?),
    };
    Ok(executor)
}

fn executor_backend_name() -> &'static str {
    match ExecutorBackendConfig::from_env() {
        Ok(ExecutorBackendConfig::Runtime(_)) => "runtime",
        _ => "noop",
    }
}

/// Factory closure that produces a [`NatsTriggerSubscriber`] once the
/// application's `AutoDispatchService` has been constructed.
type SubscriberFactory = Box<dyn FnOnce(Arc<AutoDispatchService>) -> NatsTriggerSubscriber>;

/// How long to wait for NATS to be reachable during startup.
///
/// Deployments bring NATS and the choreographer up together (compose,
/// Kubernetes, etc.). Failing fast on the first connection attempt
/// means any transient unavailability forces a restart; a bounded
/// retry is the production-correct behaviour.
const NATS_CONNECT_BUDGET: std::time::Duration = std::time::Duration::from_secs(30);

async fn connect_nats_with_retry(
    url: &str,
    total_budget: std::time::Duration,
) -> Result<async_nats::Client, ComposeError> {
    let deadline = std::time::Instant::now() + total_budget;
    let mut last_err: Option<async_nats::ConnectError> = None;
    while std::time::Instant::now() < deadline {
        match async_nats::connect(url).await {
            Ok(client) => return Ok(client),
            Err(err) => {
                tracing::warn!(url, error = %err, "nats not ready yet; retrying");
                last_err = Some(err);
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
    }
    Err(ComposeError::NatsConnect(last_err.unwrap_or_else(|| {
        // Unreachable: the loop exits only after at least one attempt.
        panic!("nats connect budget elapsed with no error recorded")
    })))
}

/// Everything `compose` needs out of the messaging wiring phase:
/// the port implementation that use cases talk through, a factory
/// for the inbound subscriber, and — when NATS is wired — a handle
/// to the live client so the health endpoints can probe it.
struct MessagingWiring {
    port: Arc<dyn MessagingPort>,
    subscriber_factory: Option<SubscriberFactory>,
    nats_client: Option<async_nats::Client>,
}

async fn wire_messaging(cfg: &ServiceConfig) -> Result<MessagingWiring, ComposeError> {
    if !cfg.nats_enabled {
        info!("nats disabled; using noop messaging");
        let port: Arc<dyn MessagingPort> = Arc::new(NoopMessaging::new());
        return Ok(MessagingWiring {
            port,
            subscriber_factory: None,
            nats_client: None,
        });
    }

    let nats_cfg = NatsConfig::new(&cfg.nats_url, &cfg.publish_prefix, &cfg.trigger_subject)?;
    let client = connect_nats_with_retry(&nats_cfg.url, NATS_CONNECT_BUDGET).await?;
    info!(url = nats_cfg.url.as_str(), "nats connected");

    let port: Arc<dyn MessagingPort> = Arc::new(NatsMessaging::new(
        client.clone(),
        nats_cfg.subjects.clone(),
    ));

    let subjects = nats_cfg.subjects.clone();
    let factory_client = client.clone();
    let subscriber_factory: SubscriberFactory =
        Box::new(move |dispatch| NatsTriggerSubscriber::new(factory_client, subjects, dispatch));

    Ok(MessagingWiring {
        port,
        subscriber_factory: Some(subscriber_factory),
        nats_client: Some(client),
    })
}

/// Composite of the persistent handles the app needs. Kept as a
/// single bag so the composition root wires them together — either
/// all backed by Postgres, or all in-memory. Splitting the source of
/// truth across replicas (half Postgres, half in-memory) is not a
/// useful configuration today.
struct Persistence {
    repository: Arc<dyn DeliberationRepositoryPort>,
    council_registry: Arc<dyn CouncilRegistryPort>,
    agent_registry: Arc<dyn AgentRegistryPort>,
    agent_resolver: Arc<dyn AgentResolverPort>,
    statistics: Arc<dyn StatisticsPort>,
}

/// Pick persistent backings based on config. When `CHOREO_POSTGRES_URL`
/// is set, every registry that has a Postgres adapter goes through
/// it; migrations apply on startup so a fresh cluster is exercisable.
/// Otherwise the in-memory defaults are wired.
async fn wire_persistence(
    cfg: &ServiceConfig,
    agent_factory: Arc<dyn AgentFactoryPort>,
) -> Result<Persistence, ComposeError> {
    if let Some(url) = cfg.postgres_url.as_deref() {
        let pool = PostgresPool::connect(&PostgresConfig::from_url(url)).await?;
        pool.run_migrations().await?;
        let agents = Arc::new(PostgresAgentRegistry::new(pool.clone(), agent_factory));
        info!("postgres persistence wired (deliberations, councils, agents, statistics)");
        Ok(Persistence {
            repository: Arc::new(PostgresDeliberationRepository::new(pool.clone())),
            council_registry: Arc::new(PostgresCouncilRegistry::new(pool.clone())),
            agent_registry: agents.clone(),
            agent_resolver: agents,
            statistics: Arc::new(PostgresStatistics::new(pool)),
        })
    } else {
        info!("postgres disabled; using in-memory persistence");
        let agents = Arc::new(InMemoryAgentRegistry::new());
        Ok(Persistence {
            repository: Arc::new(InMemoryDeliberationRepository::new()),
            council_registry: Arc::new(InMemoryCouncilRegistry::new()),
            agent_registry: agents.clone(),
            agent_resolver: agents,
            statistics: Arc::new(InMemoryStatistics::new()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use choreo_proto::runtime_v1 as runtime_pb;
    use tonic::{transport::Server, Request, Response, Status};

    #[tokio::test]
    async fn compose_builds_application_with_nats_disabled() {
        // Serialize env mutations to avoid races with other tests
        // that touch `CHOREO_*` vars (see `config::tests` for the
        // same pattern — we keep a local mutex here so we do not
        // depend on another crate's private state).
        static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
        let _guard = ENV_LOCK.lock().await;

        // Clear CHOREO_* so the defaults apply, then disable NATS
        // (so this test does not require a broker).
        for (k, _) in std::env::vars() {
            if k.starts_with("CHOREO_") {
                std::env::remove_var(k);
            }
        }
        std::env::set_var("CHOREO_NATS_ENABLED", "false");

        let app = compose().await.expect("compose should succeed");
        assert!(!app.service_config.nats_enabled);
        assert!(app.nats_subscriber.is_none());
        // The gRPC service is wired and ready but no server has started.
        let _ = &app.grpc_service;

        std::env::remove_var("CHOREO_NATS_ENABLED");
    }

    #[derive(Debug, Clone, Default)]
    struct StubRuntime;

    #[async_trait]
    impl runtime_pb::session_service_server::SessionService for StubRuntime {
        async fn create_session(
            &self,
            _request: Request<runtime_pb::CreateSessionRequest>,
        ) -> Result<Response<runtime_pb::CreateSessionResponse>, Status> {
            Ok(Response::new(runtime_pb::CreateSessionResponse {
                session: None,
            }))
        }

        async fn close_session(
            &self,
            _request: Request<runtime_pb::CloseSessionRequest>,
        ) -> Result<Response<runtime_pb::CloseSessionResponse>, Status> {
            Ok(Response::new(runtime_pb::CloseSessionResponse {
                closed: true,
            }))
        }
    }

    #[async_trait]
    impl runtime_pb::invocation_service_server::InvocationService for StubRuntime {
        async fn invoke_tool(
            &self,
            _request: Request<runtime_pb::InvokeToolRequest>,
        ) -> Result<Response<runtime_pb::InvokeToolResponse>, Status> {
            Ok(Response::new(runtime_pb::InvokeToolResponse {
                invocation: None,
            }))
        }
    }

    async fn spawn_runtime_stub() -> (std::net::SocketAddr, tokio::sync::oneshot::Sender<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

        tokio::spawn(async move {
            Server::builder()
                .add_service(
                    runtime_pb::session_service_server::SessionServiceServer::new(StubRuntime),
                )
                .add_service(
                    runtime_pb::invocation_service_server::InvocationServiceServer::new(
                        StubRuntime,
                    ),
                )
                .serve_with_incoming_shutdown(incoming, async move {
                    let _ = shutdown_rx.await;
                })
                .await
                .unwrap();
        });

        (addr, shutdown_tx)
    }

    #[tokio::test]
    async fn compose_builds_application_with_runtime_executor_selected() {
        static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
        let _guard = ENV_LOCK.lock().await;

        for (k, _) in std::env::vars() {
            if k.starts_with("CHOREO_") {
                std::env::remove_var(k);
            }
        }

        let (addr, shutdown) = spawn_runtime_stub().await;
        std::env::set_var("CHOREO_NATS_ENABLED", "false");
        std::env::set_var("CHOREO_EXECUTOR_KIND", "runtime");
        std::env::set_var("CHOREO_RUNTIME_GRPC_ENDPOINT", format!("http://{addr}"));

        let app = compose().await.expect("compose should succeed");
        assert!(!app.service_config.nats_enabled);
        assert!(app.nats_subscriber.is_none());

        let _ = shutdown.send(());
        std::env::remove_var("CHOREO_NATS_ENABLED");
        std::env::remove_var("CHOREO_EXECUTOR_KIND");
        std::env::remove_var("CHOREO_RUNTIME_GRPC_ENDPOINT");
    }
}
