//! Wire every adapter and use case into a runnable [`Application`].

use std::sync::Arc;

use made_adapters::agents::DispatchingAgentFactory;
use made_adapters::ceremony::DeliberatingCeremonyStepHandler;
use made_adapters::clock::SystemClock;
use made_adapters::config::EnvConfiguration;
use made_adapters::memory::InMemoryCeremonyDefinitionRepository;
use made_adapters::metrics::PrometheusMetricsRecorder;
use made_adapters::progress::CeremonyProgressNotifier;

use made_app::services::{AutoDispatchService, SessionMemoryRecorder, SessionStream};
use made_app::usecases::{
    AcceptChildCompletionUseCase, CreateCouncilUseCase, DeleteCouncilUseCase, DeliberateUseCase,
    GetDeliberationUseCase, ListCouncilsUseCase, OrchestrateUseCase,
    PrepareCeremonyChildrenUseCase, PrepareCeremonyParticipantsUseCase,
    RecoverCeremonyChildrenUseCase, RegisterAgentUseCase, ResolveCeremonyDefinitionUseCase,
    RunCeremonyStepUseCase, RunCeremonyUseCase, RunCouncilDecisionUseCase,
    StartCeremonyStepUseCase, StartCeremonyUseCase, StartPublishedCeremonyUseCase,
    UnregisterAgentUseCase,
};
use made_core::ports::{
    AgentFactoryPort, CeremonyDefinitionRepositoryPort, CeremonyStepHandlerPort, ScoringPort,
};
use tracing::info;

use crate::{Application, ComposeError};

use messaging::{wire_messaging, MessagingWiring};

use ceremony_lifecycle::CeremonyLifecycleControls;
use ceremony_persistence::{wire as wire_ceremony_persistence, CeremonyPersistence};
use persistence::wire_persistence;
use persistence_handles::Persistence;

mod artifact_storage;
mod ceremony_lifecycle;
mod ceremony_operations;
mod ceremony_persistence;
mod ceremony_publisher;
mod ceremony_queries;
mod execution_receipts;
mod executor;
mod messaging;
mod persistence;
mod persistence_handles;
mod scoring;
mod validators;

/// Wire configured adapters and in-memory defaults into the runnable application.
/// Explicit environment settings select richer persistence, messaging, execution,
/// and seed adapters; omitted integrations retain their documented defaults.
#[allow(clippy::too_many_lines)]
pub async fn compose() -> Result<Application, ComposeError> {
    let service_config = EnvConfiguration::new().load()?;

    let clock = Arc::new(SystemClock::new());
    // One Prometheus registry for use cases and the health endpoint. Fails
    // fast if a metric is malformed (a wiring bug).
    let metrics_recorder = Arc::new(PrometheusMetricsRecorder::new()?);
    let mut validators = validators::wire(metrics_recorder.clone())?;
    // Choose scoring, and when an LLM judge is configured append it to
    // the validator chain so its verdict drives the ranking.
    let scoring: Arc<dyn ScoringPort> = scoring::wire(&mut validators, metrics_recorder.clone())?;
    let executor = executor::wire().await?;
    let dispatching_factory =
        DispatchingAgentFactory::from_env()?.with_metrics(metrics_recorder.clone());
    let supported_agent_kinds = dispatching_factory.supported_kinds().join(",");
    let agent_factory: Arc<dyn AgentFactoryPort> = Arc::new(dispatching_factory);

    // Select council repositories and their journal together to keep one source of truth.
    let Persistence {
        repository,
        council_registry,
        agent_registry,
        agent_resolver,
        statistics,
        contract_registry,
        council_journal,
        pool: postgres_pool,
    } = wire_persistence(&service_config, agent_factory.clone()).await?;
    let artifacts = artifact_storage::wire(&service_config, postgres_pool.as_ref())?;

    let ceremony_definitions: Arc<dyn CeremonyDefinitionRepositoryPort> =
        Arc::new(InMemoryCeremonyDefinitionRepository::new());
    let CeremonyPersistence {
        events: ceremony_events,
        cursors: ceremony_cursors,
        snapshots: ceremony_snapshots,
        publications: ceremony_publications,
        memory_writer,
        memory_reader,
        receipts: execution_receipts,
    } = wire_ceremony_persistence(&service_config, postgres_pool.as_ref())?;
    // The writer is a subscriber of the stream: memory is a projection
    // of sealed events, outside the ceremony transaction (ADR-012/013).
    let session_memory = Arc::new(SessionMemoryRecorder::new(
        memory_writer,
        ceremony_events.clone(),
    ));
    let MessagingWiring {
        port: messaging_transport,
        subscriber_factory: nats_subscriber_factory,
        ceremony_recovery_factory,
        nats_client,
        ceremony_transport,
    } = wire_messaging(&service_config, metrics_recorder.clone()).await?;
    let council_event_publisher = service_config.nats_enabled.then(|| {
        Arc::new(made_app::usecases::PublishCouncilEventsUseCase::new(
            council_journal.clone(),
            messaging_transport,
            clock.clone(),
        ))
    });
    let messaging = Arc::new(
        made_adapters::council_journal_messaging::CouncilJournalMessaging::new(
            council_journal.clone(),
        ),
    );
    let event_publisher = ceremony_publisher::wire(
        ceremony_events.clone(),
        ceremony_cursors.clone(),
        ceremony_transport,
        clock.clone(),
    )
    .await?;
    let progress_notifier = Arc::new(CeremonyProgressNotifier::new());
    let subscribers = ceremony_publisher::subscribers(
        session_memory,
        progress_notifier.clone(),
        ceremony_events.clone(),
        metrics_recorder.clone(),
        event_publisher,
    );
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
    // Resolve bound definitions from the catalog and unbound definitions from the repository.
    let resolve_ceremony_definition = Arc::new(ResolveCeremonyDefinitionUseCase::new(
        ceremony_definitions.clone(),
        ceremony_publications.clone(),
    ));
    // Both run modes use the same stream to carry step results forward.
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
        memory_reader.clone(),
    ));
    let prepare_ceremony_children = Arc::new(PrepareCeremonyChildrenUseCase::new(
        resolve_ceremony_definition.clone(),
        ceremony_publications.clone(),
        ceremony_stream.clone(),
        clock.clone(),
        memory_reader,
    ));
    let run_ceremony = Arc::new(
        RunCeremonyUseCase::new(
            ceremony_definitions.clone(),
            ceremony_stream.clone(),
            ceremony_step_handler.clone(),
            clock.clone(),
        )
        .with_metrics(metrics_recorder.clone())
        .with_max_parallel_ceiling(service_config.max_parallel)
        .with_child_orchestrator(prepare_ceremony_children.clone()),
    );
    let accept_child_completion = Arc::new(AcceptChildCompletionUseCase::new(
        resolve_ceremony_definition.clone(),
        ceremony_publications.clone(),
        ceremony_stream.clone(),
        clock.clone(),
    ));
    let recover_ceremony_children = Arc::new(RecoverCeremonyChildrenUseCase::new(
        ceremony_events.clone(),
        ceremony_cursors.clone(),
        ceremony_stream.clone(),
        prepare_ceremony_children.clone(),
        accept_child_completion.clone(),
        clock.clone(),
        made_core::value_objects::CeremonyEventConsumer::new("made.children.recovery.v1")?,
    ));
    ceremony_operations::recover_to_head(&recover_ceremony_children).await?;
    let run_ceremony_step = Arc::new(
        RunCeremonyStepUseCase::new(
            resolve_ceremony_definition.clone(),
            ceremony_stream.clone(),
            ceremony_step_handler,
            clock.clone(),
        )
        .with_max_parallel_ceiling(service_config.max_parallel)
        .with_child_orchestrator(prepare_ceremony_children),
    );
    // Delegated hosts claim through the same use case as embedded execution.
    let claim_ceremony_step = Arc::new(
        StartCeremonyStepUseCase::new(
            resolve_ceremony_definition.clone(),
            ceremony_stream.clone(),
            clock.clone(),
        )
        .with_max_parallel_ceiling(service_config.max_parallel),
    );

    let lifecycle = CeremonyLifecycleControls::wire(
        resolve_ceremony_definition.clone(),
        ceremony_stream.clone(),
        clock.clone(),
    );

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
    let nats_ceremony_recovery =
        ceremony_recovery_factory.map(|factory| factory(recover_ceremony_children.clone()));

    let mut grpc_builder = made_adapters::grpc::MadeGrpcService::builder()
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
        .accept_child_completion(accept_child_completion)
        .recover_ceremony_children(recover_ceremony_children)
        .claim_ceremony_step(claim_ceremony_step)
        .pause_ceremony(lifecycle.pause)
        .resume_ceremony(lifecycle.resume)
        .cancel_ceremony(lifecycle.cancel)
        .enforce_ceremony_deadlines(lifecycle.enforce_deadlines)
        .ceremony_definitions(ceremony_definitions.clone())
        .resolve_ceremony_definition(resolve_ceremony_definition.clone())
        .prepare_ceremony_participants(prepare_ceremony_participants)
        .contract_registry(contract_registry.clone())
        .auto_dispatch(auto_dispatch)
        .statistics(statistics.clone())
        // The same registry the use cases record into and `/metrics`
        // renders, so `GetStatus` names what is actually recording.
        .observability(metrics_recorder.clone())
        .service_version(env!("CARGO_PKG_VERSION"))
        .council_journal(Arc::new(made_app::services::CouncilJournalService::new(
            council_journal,
            clock.clone(),
        )))
        .clock(clock.clone())
        .max_parallel_ceiling(service_config.max_parallel);
    grpc_builder = ceremony_queries::wire(
        grpc_builder,
        resolve_ceremony_definition.clone(),
        ceremony_stream.clone(),
        ceremony_events,
        ceremony_cursors,
        progress_notifier,
        ceremony_publications,
    );
    grpc_builder = execution_receipts::wire(
        grpc_builder,
        resolve_ceremony_definition.clone(),
        ceremony_stream.clone(),
        execution_receipts,
        clock.clone(),
        artifacts.clone(),
    );
    grpc_builder = ceremony_operations::wire(
        grpc_builder,
        resolve_ceremony_definition,
        ceremony_stream,
        clock,
    );
    if let Some(artifacts) = artifacts {
        grpc_builder = grpc_builder.artifacts(artifacts);
    }
    let grpc_service = grpc_builder.build()?;

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
        executor_backend = executor::backend_name(),
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
        council_event_publisher,
        nats_subscriber,
        nats_ceremony_recovery,
        health_state,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use made_proto::runtime_v1 as runtime_pb;
    use tonic::{transport::Server, Request, Response, Status};

    // One lock prevents concurrent tests racing over process-wide MADE_* variables.
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
