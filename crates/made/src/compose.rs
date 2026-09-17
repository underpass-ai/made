//! Wire every adapter and use case into a runnable [`Application`].

use std::sync::Arc;

use made_adapters::agents::DispatchingAgentFactory;
use made_adapters::ceremony::DeliberatingCeremonyStepHandler;
use made_adapters::clock::SystemClock;
use made_adapters::config::{EnvConfiguration, ServiceConfig};
use made_adapters::memory::{
    InMemoryAgentRegistry, InMemoryCeremonyDefinitionRepository, InMemoryContractRegistry,
    InMemoryCouncilRegistry, InMemoryDeliberationRepository, InMemoryStatistics,
};
use made_adapters::metrics::PrometheusMetricsRecorder;
use made_adapters::noop::{NoopCeremonyEvidenceSource, NoopExecutor};
use made_adapters::postgres::{
    PostgresAgentRegistry, PostgresConfig, PostgresCouncilRegistry, PostgresDeliberationRepository,
    PostgresPool, PostgresStatistics,
};
use made_adapters::runtime::{ExecutorBackendConfig, RuntimeExecutor};
use made_adapters::scoring::{JudgeAwareScoring, UniformScoring};
use made_adapters::validators::{
    AllowedStringValuesValidator, BoundedEventShapeValidator, ClaimsEvidenceGroundedValidator,
    ClaimsEvidenceSupportedValidator, ContentNonEmptyValidator, JsonObjectOutputValidator,
    JsonSchemaValidator, RequiredFieldsValidator,
};
use made_app::services::{
    AutoDispatchService, CeremonyEventFanout, CeremonyEventPublisherSubscriber,
    SessionMemoryRecorder, SessionStream,
};
use made_app::usecases::{
    ApplyCeremonyTransitionUseCase, ApproveCeremonyGuardUseCase, AssertCeremonyReasonUseCase,
    BindCeremonyParticipantsUseCase, CloseCeremonyInterventionUseCase,
    CollectCeremonyEvidenceUseCase, CompleteCeremonyStepUseCase, CreateCouncilUseCase,
    DeferCeremonyGuardUseCase, DeleteCouncilUseCase, DeliberateUseCase,
    DiffCeremonyDefinitionsUseCase, GenerateCeremonyReportUseCase, GetCeremonyInstanceUseCase,
    GetCeremonyTranscriptUseCase, GetDeliberationUseCase, ListCeremonyInstancesUseCase,
    ListCouncilsUseCase, OrchestrateUseCase, PrepareCeremonyParticipantsUseCase,
    PublishCeremonyDefinitionUseCase, PublishCeremonyEventsUseCase, PullCeremonyEventsUseCase,
    ReadCeremonyEventsUseCase, RegisterAgentUseCase, RequestCeremonyInterventionUseCase,
    ResolveCeremonyDefinitionUseCase, RespondToCeremonyInterventionUseCase, RunCeremonyStepUseCase,
    RunCeremonyUseCase, RunCouncilDecisionUseCase, StartCeremonyStepUseCase, StartCeremonyUseCase,
    StartPublishedCeremonyUseCase, UnregisterAgentUseCase, VerifyCeremonyJournalUseCase,
};
use made_core::error::DomainError;
use made_core::ports::{
    AgentFactoryPort, AgentRegistryPort, AgentResolverPort, CeremonyDefinitionRepositoryPort,
    CeremonyEventSubscriberPort, CeremonyStepHandlerPort, ContractRegistryPort,
    CouncilRegistryPort, DeliberationRepositoryPort, ExecutorPort, MetricsRecorderPort,
    ScoringPort, StatisticsPort, ValidatorPort,
};
use tracing::info;

use crate::{Application, ComposeError};

use messaging::{wire_messaging, MessagingWiring};

use ceremony_persistence::{wire as wire_ceremony_persistence, CeremonyPersistence};

mod ceremony_persistence;
mod messaging;

/// Pick the scoring policy and wire the optional LLM judge.
///
/// When `judge_from_env` yields a judge it is appended to `validators`,
/// and `JudgeAwareScoring` makes its verdict rank proposals; otherwise
/// scoring is uniform. Fails fast when the judge is enabled but
/// misconfigured.
fn wire_scoring(
    validators: &mut Vec<Arc<dyn ValidatorPort>>,
    metrics: Arc<dyn MetricsRecorderPort>,
) -> Result<Arc<dyn ScoringPort>, DomainError> {
    match made_adapters::agents::judge_from_env(metrics.clone())? {
        Some(judge) => {
            validators.push(judge);
            info!("scoring: LLM judge enabled; ranking by judge verdict");
            Ok(Arc::new(JudgeAwareScoring::new().with_metrics(metrics)))
        }
        None => Ok(Arc::new(UniformScoring::new())),
    }
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
/// - Optionally seeds demo councils if `MADE_SEED_SPECIALTIES` is
///   set, so an empty deployment is immediately exercisable against
///   the AsyncAPI / gRPC contract.
#[allow(clippy::too_many_lines)]
pub async fn compose() -> Result<Application, ComposeError> {
    let service_config = EnvConfiguration::new().load()?;

    let clock = Arc::new(SystemClock::new());
    // One Prometheus registry for the whole process, shared between the
    // use cases that record into it and the health endpoint that renders
    // it. Fails fast if a metric is malformed (a wiring bug).
    let metrics_recorder = Arc::new(PrometheusMetricsRecorder::new()?);
    let mut validators: Vec<Arc<dyn ValidatorPort>> = vec![
        Arc::new(ContentNonEmptyValidator::new()),
        Arc::new(JsonObjectOutputValidator::new()),
        Arc::new(RequiredFieldsValidator::new()),
        Arc::new(AllowedStringValuesValidator::new()),
        Arc::new(JsonSchemaValidator::new()),
        // Evidence grounding: rejects claims citing refs outside the
        // contract's evidence pack (no-op unless a contract declares a
        // grounding rule). Runs before the shape-budget guard so orphan
        // refs are named even when the output is otherwise well-formed.
        Arc::new(ClaimsEvidenceGroundedValidator::new()),
        // Semantic support: rejects claims whose *cited* evidence does
        // not actually support them, judged through the deployment's
        // evidence-support judge (`MADE_SUPPORT_JUDGE_ENABLED`, vLLM
        // endpoint/model). No-op unless a contract declares
        // `evidence.semantic_support`; a contract that demands it with
        // no judge wired fails its step loudly instead of running the
        // gate voided.
        Arc::new(ClaimsEvidenceSupportedValidator::new(
            made_adapters::agents::support_judge_from_env(metrics_recorder.clone())?,
        )),
        // Final shape-budget guard: defends downstream bus consumers
        // against pathological JSON (deeply nested, huge arrays,
        // bloated strings). Uses the validator's conservative
        // defaults; tune with the `with_*` builders when a deploy
        // needs different bounds.
        Arc::new(BoundedEventShapeValidator::new()),
    ];
    // Choose scoring, and when an LLM judge is configured append it to
    // the validator chain so its verdict drives the ranking.
    let scoring: Arc<dyn ScoringPort> = wire_scoring(&mut validators, metrics_recorder.clone())?;
    let executor = wire_executor().await?;
    let dispatching_factory =
        DispatchingAgentFactory::from_env()?.with_metrics(metrics_recorder.clone());
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
        pool: postgres_pool,
    } = wire_persistence(&service_config, agent_factory.clone()).await?;

    // The contract registry is in-memory only today: contracts are
    // small, stable, and seeded from `MADE_CONTRACT_DIR` so the
    // operator's source of truth lives on disk. When Postgres-backed
    // contracts land it joins `Persistence` above.
    let contract_registry: Arc<dyn ContractRegistryPort> =
        Arc::new(InMemoryContractRegistry::new());
    let ceremony_definitions: Arc<dyn CeremonyDefinitionRepositoryPort> =
        Arc::new(InMemoryCeremonyDefinitionRepository::new());
    let CeremonyPersistence {
        events: ceremony_events,
        cursors: ceremony_cursors,
        snapshots: ceremony_snapshots,
        publications: ceremony_publications,
        memory_writer,
        memory_reader,
    } = wire_ceremony_persistence(&service_config)?;
    // The writer is a subscriber of the stream: memory is a projection
    // of sealed events, outside the ceremony transaction (ADR-012/013).
    let session_memory = Arc::new(SessionMemoryRecorder::new(
        memory_writer,
        ceremony_events.clone(),
    ));
    let MessagingWiring {
        port: messaging,
        subscriber_factory: nats_subscriber_factory,
        nats_client,
        ceremony_transport,
    } = wire_messaging(&service_config, metrics_recorder.clone()).await?;
    let publisher_consumer = made_core::value_objects::CeremonyEventConsumer::new("nats-publisher")
        .expect("the NATS publisher consumer name is valid");
    let publisher_use_case = ceremony_transport.map(|transport| {
        Arc::new(PublishCeremonyEventsUseCase::new(
            ceremony_events.clone(),
            ceremony_cursors.clone(),
            transport,
            clock.clone(),
        ))
    });
    if let Some(publisher) = &publisher_use_case {
        loop {
            let round = publisher
                .execute(
                    &publisher_consumer,
                    made_core::value_objects::CeremonyEventPageLimit::DEFAULT,
                )
                .await?;
            if round.busy
                || round.failed > 0
                || round.delivered + round.quarantined
                    < made_core::value_objects::CeremonyEventPageLimit::DEFAULT.value()
            {
                break;
            }
        }
    }
    let event_publisher = publisher_use_case.map(|publisher| {
        Arc::new(CeremonyEventPublisherSubscriber::new(
            publisher,
            publisher_consumer,
        )) as Arc<dyn CeremonyEventSubscriberPort>
    });
    let subscribers = Arc::new(CeremonyEventFanout::new(
        core::iter::once(session_memory as Arc<dyn CeremonyEventSubscriberPort>)
            .chain(event_publisher)
            .collect(),
    ));
    let ceremony_stream = Arc::new(SessionStream::new(
        ceremony_events.clone(),
        ceremony_snapshots,
        subscribers,
    ));

    let deliberate = Arc::new(DeliberateUseCase::new(
        clock.clone(),
        council_registry.clone(),
        agent_resolver.clone(),
        validators,
        scoring,
        repository.clone(),
        messaging.clone(),
        statistics.clone(),
        metrics_recorder.clone(),
        "made",
    ));

    let ceremony_step_handler: Arc<dyn CeremonyStepHandlerPort> =
        Arc::new(DeliberatingCeremonyStepHandler::new(deliberate.clone()));

    let orchestrate = Arc::new(OrchestrateUseCase::new(
        deliberate.clone(),
        executor,
        messaging.clone(),
        clock.clone(),
        statistics.clone(),
        "made",
    ));

    let run_council_decision = Arc::new(RunCouncilDecisionUseCase::new(
        contract_registry.clone(),
        council_registry.clone(),
        deliberate.clone(),
        repository.clone(),
    ));
    let run_ceremony = Arc::new(
        RunCeremonyUseCase::new(
            ceremony_definitions.clone(),
            ceremony_stream.clone(),
            ceremony_step_handler.clone(),
            clock.clone(),
        )
        .with_metrics(metrics_recorder.clone()),
    );
    // How every verb that advances a session finds what it is running:
    // from the catalogue when the session is bound to a published
    // version, from the repository when it is not.
    let resolve_ceremony_definition = Arc::new(ResolveCeremonyDefinitionUseCase::new(
        ceremony_definitions.clone(),
        ceremony_publications.clone(),
    ));
    // Advancing a session one move at a time. What a step said reaches
    // the next step through the stream both drivers append to, so it
    // is there whichever way the run was driven.
    let start_ceremony = Arc::new(StartCeremonyUseCase::new(
        ceremony_definitions.clone(),
        ceremony_stream.clone(),
        clock.clone(),
        memory_reader.clone(),
    ));
    let start_published_ceremony = Arc::new(StartPublishedCeremonyUseCase::new(
        ceremony_publications.clone(),
        ceremony_stream.clone(),
        clock.clone(),
        memory_reader,
    ));
    let run_ceremony_step = Arc::new(RunCeremonyStepUseCase::new(
        resolve_ceremony_definition.clone(),
        ceremony_stream.clone(),
        ceremony_step_handler,
        clock.clone(),
    ));
    // The delegated-host protocol. Claiming and completing are the
    // same two use cases the embedded edition has always called; only
    // the way in is new.
    let claim_ceremony_step = Arc::new(StartCeremonyStepUseCase::new(
        resolve_ceremony_definition.clone(),
        ceremony_stream.clone(),
        clock.clone(),
    ));
    let complete_ceremony_step = Arc::new(CompleteCeremonyStepUseCase::new(
        resolve_ceremony_definition.clone(),
        ceremony_stream.clone(),
        clock.clone(),
    ));
    let apply_ceremony_transition = Arc::new(ApplyCeremonyTransitionUseCase::new(
        resolve_ceremony_definition.clone(),
        ceremony_stream.clone(),
        clock.clone(),
    ));
    let assert_ceremony_reason = Arc::new(AssertCeremonyReasonUseCase::new(
        resolve_ceremony_definition.clone(),
        ceremony_stream.clone(),
        clock.clone(),
    ));
    let approve_ceremony_guard = Arc::new(ApproveCeremonyGuardUseCase::new(
        resolve_ceremony_definition.clone(),
        ceremony_stream.clone(),
        clock.clone(),
    ));
    let defer_ceremony_guard = Arc::new(DeferCeremonyGuardUseCase::new(
        resolve_ceremony_definition.clone(),
        ceremony_stream.clone(),
        clock.clone(),
    ));
    let request_ceremony_intervention = Arc::new(RequestCeremonyInterventionUseCase::new(
        resolve_ceremony_definition.clone(),
        ceremony_stream.clone(),
        clock.clone(),
    ));
    let respond_to_ceremony_intervention = Arc::new(RespondToCeremonyInterventionUseCase::new(
        resolve_ceremony_definition.clone(),
        ceremony_stream.clone(),
        clock.clone(),
    ));
    let close_ceremony_intervention = Arc::new(CloseCeremonyInterventionUseCase::new(
        resolve_ceremony_definition.clone(),
        ceremony_stream.clone(),
        clock.clone(),
    ));
    // No evidence source ships with the server, so this answers
    // NOT_FOUND until an operator wires one. Failing plainly beats
    // a missing method or an invented answer.
    let collect_ceremony_evidence = Arc::new(CollectCeremonyEvidenceUseCase::new(
        resolve_ceremony_definition.clone(),
        ceremony_stream.clone(),
        Arc::new(NoopCeremonyEvidenceSource::new()),
        clock.clone(),
    ));
    let publish_ceremony_definition = Arc::new(PublishCeremonyDefinitionUseCase::new(
        ceremony_publications.clone(),
    ));
    let diff_ceremony_definitions = Arc::new(DiffCeremonyDefinitionsUseCase::new(
        ceremony_publications.clone(),
    ));
    let bind_ceremony_participants = Arc::new(BindCeremonyParticipantsUseCase::new(
        resolve_ceremony_definition.clone(),
        ceremony_stream.clone(),
        clock.clone(),
    ));

    let create_council = Arc::new(CreateCouncilUseCase::new(
        clock.clone(),
        council_registry.clone(),
        agent_resolver.clone(),
    ));
    let prepare_ceremony_participants = Arc::new(PrepareCeremonyParticipantsUseCase::new(
        clock.clone(),
        agent_factory.clone(),
        agent_registry.clone(),
        council_registry.clone(),
    ));
    let delete_council = Arc::new(DeleteCouncilUseCase::new(council_registry.clone()));
    let list_councils = Arc::new(ListCouncilsUseCase::new(council_registry.clone()));
    let get_deliberation = Arc::new(GetDeliberationUseCase::new(repository.clone()));

    let register_agent = Arc::new(RegisterAgentUseCase::new(
        agent_factory.clone(),
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
    crate::seeding::apply_contract_seeding(contract_registry.as_ref()).await?;

    // Now that the auto-dispatch service exists, the subscriber
    // factory can finish wiring.
    let nats_subscriber = nats_subscriber_factory.map(|factory| factory(auto_dispatch.clone()));

    let get_ceremony_instance = Arc::new(GetCeremonyInstanceUseCase::new(ceremony_stream.clone()));
    let list_ceremony_instances =
        Arc::new(ListCeremonyInstancesUseCase::new(ceremony_stream.clone()));
    // What a session left behind. All three read the stream: the page
    // hands back records, the transcript folds what the steps said out
    // of them, and the report composes the session, its definition and
    // its stream into the one projection both editions render
    // (ADR-006, ADR-012).
    let read_ceremony_events = Arc::new(ReadCeremonyEventsUseCase::new(ceremony_events.clone()));
    let pull_ceremony_events = Arc::new(PullCeremonyEventsUseCase::new(
        ceremony_events.clone(),
        ceremony_cursors,
    ));
    let verify_ceremony_journal =
        Arc::new(VerifyCeremonyJournalUseCase::new(ceremony_events.clone()));
    let get_ceremony_transcript =
        Arc::new(GetCeremonyTranscriptUseCase::new(ceremony_events.clone()));
    let generate_ceremony_report = Arc::new(GenerateCeremonyReportUseCase::new(
        get_ceremony_instance.clone(),
        resolve_ceremony_definition.clone(),
        ceremony_events,
    ));

    let grpc_service = made_adapters::grpc::MadeGrpcService::builder()
        .deliberate(deliberate)
        .orchestrate(orchestrate)
        .create_council(create_council)
        .delete_council(delete_council)
        .list_councils(list_councils)
        .get_deliberation(get_deliberation)
        .register_agent(register_agent)
        .unregister_agent(unregister_agent)
        .run_council_decision(run_council_decision)
        .run_ceremony(run_ceremony)
        .start_ceremony(start_ceremony)
        .start_published_ceremony(start_published_ceremony)
        .run_ceremony_step(run_ceremony_step)
        .claim_ceremony_step(claim_ceremony_step)
        .complete_ceremony_step(complete_ceremony_step)
        .apply_ceremony_transition(apply_ceremony_transition)
        .approve_ceremony_guard(approve_ceremony_guard)
        .defer_ceremony_guard(defer_ceremony_guard)
        .assert_ceremony_reason(assert_ceremony_reason)
        .request_ceremony_intervention(request_ceremony_intervention)
        .respond_to_ceremony_intervention(respond_to_ceremony_intervention)
        .close_ceremony_intervention(close_ceremony_intervention)
        .collect_ceremony_evidence(collect_ceremony_evidence)
        .read_ceremony_events(read_ceremony_events)
        .pull_ceremony_events(pull_ceremony_events)
        .verify_ceremony_journal(verify_ceremony_journal)
        .get_ceremony_transcript(get_ceremony_transcript)
        .generate_ceremony_report(generate_ceremony_report)
        .publish_ceremony_definition(publish_ceremony_definition)
        .diff_ceremony_definitions(diff_ceremony_definitions)
        .bind_ceremony_participants(bind_ceremony_participants)
        .ceremony_definitions(ceremony_definitions.clone())
        .get_ceremony_instance(get_ceremony_instance)
        .list_ceremony_instances(list_ceremony_instances)
        .resolve_ceremony_definition(resolve_ceremony_definition.clone())
        .prepare_ceremony_participants(prepare_ceremony_participants)
        .contract_registry(contract_registry.clone())
        .auto_dispatch(auto_dispatch)
        .statistics(statistics.clone())
        // The same registry the use cases record into and `/metrics`
        // renders, so `GetStatus` names what is actually recording.
        .metrics(metrics_recorder.clone())
        .service_version(env!("CARGO_PKG_VERSION"))
        .build()?;

    let health_state = crate::health::HealthState::new(
        nats_client,
        postgres_pool,
        statistics.clone(),
        metrics_recorder.clone(),
        env!("CARGO_PKG_VERSION"),
    );

    info!(
        grpc_port = service_config.grpc_port,
        http_port = service_config.http_port,
        nats_enabled = service_config.nats_enabled,
        executor_backend = executor_backend_name(),
        agent_kinds = supported_agent_kinds.as_str(),
        trigger_subject = service_config.trigger_subject.as_str(),
        "made wired"
    );

    Ok(Application {
        service_config,
        agent_registry,
        agent_resolver,
        council_registry,
        contract_registry,
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
    /// `Some` when Postgres-backed, so the readiness probe can check the
    /// database; `None` for in-memory persistence.
    pool: Option<PostgresPool>,
}

/// Pick persistent backings based on config. When `MADE_POSTGRES_URL`
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
            statistics: Arc::new(PostgresStatistics::new(pool.clone())),
            pool: Some(pool),
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
            pool: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use made_proto::runtime_v1 as runtime_pb;
    use tonic::{transport::Server, Request, Response, Status};

    // Shared across every test in this module so concurrent MADE_*
    // env mutations cannot race each other. Previously each test held
    // its own per-fn static, which serialised the test against itself
    // but did nothing across tests — under cargo's default parallel
    // runner the two `compose_builds_application_*` tests then
    // clobbered each other's vars, producing flaky NATS DNS lookups
    // in CI.
    static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    #[tokio::test]
    async fn compose_builds_application_with_nats_disabled() {
        let _guard = ENV_LOCK.lock().await;

        // Clear MADE_* so the defaults apply, then disable NATS
        // (so this test does not require a broker).
        for (k, _) in std::env::vars() {
            if k.starts_with("MADE_") {
                std::env::remove_var(k);
            }
        }
        std::env::set_var("MADE_NATS_ENABLED", "false");

        let app = compose().await.expect("compose should succeed");
        assert!(!app.service_config.nats_enabled);
        assert!(app.nats_subscriber.is_none());
        // The gRPC service is wired and ready but no server has started.
        let _ = &app.grpc_service;

        std::env::remove_var("MADE_NATS_ENABLED");
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
        let _guard = ENV_LOCK.lock().await;

        for (k, _) in std::env::vars() {
            if k.starts_with("MADE_") {
                std::env::remove_var(k);
            }
        }

        let (addr, shutdown) = spawn_runtime_stub().await;
        std::env::set_var("MADE_NATS_ENABLED", "false");
        std::env::set_var("MADE_EXECUTOR_KIND", "runtime");
        std::env::set_var("MADE_RUNTIME_GRPC_ENDPOINT", format!("http://{addr}"));

        let app = compose().await.expect("compose should succeed");
        assert!(!app.service_config.nats_enabled);
        assert!(app.nats_subscriber.is_none());

        let _ = shutdown.send(());
        std::env::remove_var("MADE_NATS_ENABLED");
        std::env::remove_var("MADE_EXECUTOR_KIND");
        std::env::remove_var("MADE_RUNTIME_GRPC_ENDPOINT");
    }
}
