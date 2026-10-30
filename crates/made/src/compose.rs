//! Wire every adapter and use case into a runnable [`Application`].

use std::sync::Arc;

use made_adapters::agents::DispatchingAgentFactory;
use made_adapters::clock::SystemClock;
use made_adapters::config::EnvConfiguration;
use made_adapters::metrics::PrometheusMetricsRecorder;
use made_adapters::progress::ProtocolProgressNotifier;

use made_app::services::AutoDispatchService;
use made_app::usecases::{RunProtocolStepUseCase, RunProtocolUseCase, StartProtocolStepUseCase};
use made_core::ports::{AgentFactoryPort, ScoringPort};

use crate::{Application, ComposeError};

use messaging::{wire_messaging, MessagingWiring};
use persistence::wire_persistence;
use persistence_handles::Persistence;
use protocol_lifecycle::ProtocolLifecycleControls;
use protocol_persistence::{wire as wire_protocol_persistence, ProtocolPersistence};

mod agentic_system;
mod artifact_storage;
mod authorization;
mod budget_operations;
mod children_recovery;
mod council_dependencies;
mod council_event_publisher;
mod council_operations;
mod execution_receipts;
mod executor;
mod host_delivery;
mod integrator_loop;
mod intervention_delivery;
mod messaging;
mod persistence;
mod persistence_handles;
mod protocol_agent_status;
mod protocol_definitions;
mod protocol_lifecycle;
mod protocol_openings;
mod protocol_operations;
mod protocol_persistence;
mod protocol_publisher;
mod protocol_queries;
mod protocol_workers;
mod registry_operations;
mod runtime_log;
mod scoring;
mod validators;

/// Wire configured adapters and in-memory defaults into the runnable application.
/// Explicit environment settings select richer persistence, messaging, execution,
/// and seed adapters; omitted integrations retain their documented defaults.
#[allow(clippy::too_many_lines)]
pub async fn compose() -> Result<Application, ComposeError> {
    let service_config = EnvConfiguration::new().load()?;

    let clock = Arc::new(SystemClock::new());
    let metrics_recorder = Arc::new(PrometheusMetricsRecorder::new()?);
    let mut validators = validators::wire(metrics_recorder.clone())?;
    // An optional LLM judge joins validators so its verdict drives ranking.
    let scoring: Arc<dyn ScoringPort> = scoring::wire(&mut validators, metrics_recorder.clone())?;
    let executor = executor::wire().await?;
    let dispatching_factory =
        DispatchingAgentFactory::from_env()?.with_metrics(metrics_recorder.clone());
    let supported_agent_kinds = dispatching_factory.supported_kinds().join(",");
    let agent_factory: Arc<dyn AgentFactoryPort> = Arc::new(dispatching_factory);

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
    let host_delivery = host_delivery::wire(&service_config, postgres_pool.as_ref())?;
    let agentic_system = agentic_system::wire(&service_config, postgres_pool.as_ref())?;
    let authorization =
        authorization::wire(&service_config, postgres_pool.as_ref(), clock.clone()).await?;
    let authorization_continuation = authorization.continuation.clone();
    let renewal_authorization = authorization.authorize.clone();

    let protocol_definitions = protocol_definitions::wire();
    let ProtocolPersistence {
        events: protocol_events,
        index: protocol_index,
        cursors: protocol_cursors,
        snapshots: protocol_snapshots,
        publications: protocol_publications,
        memory_writer,
        memory_reader,
        receipts: execution_receipts,
        budgets: budget_ledger,
    } = wire_protocol_persistence(&service_config, postgres_pool.as_ref())?;
    let MessagingWiring {
        port: messaging_transport,
        subscriber_factory: nats_subscriber_factory,
        protocol_recovery_factory,
        nats_client,
        protocol_transport,
    } = wire_messaging(&service_config, metrics_recorder.clone()).await?;
    let council_event_publisher = council_event_publisher::wire(
        service_config.nats_enabled,
        council_journal.clone(),
        messaging_transport,
        clock.clone(),
    );
    let messaging = council_event_publisher::journal_messaging(council_journal.clone());
    let event_publisher = protocol_publisher::wire(
        protocol_events.clone(),
        protocol_cursors.clone(),
        protocol_transport,
        clock.clone(),
    )
    .await?;
    let attention_recovery = integrator_loop::recovery(
        protocol_events.clone(),
        protocol_cursors.clone(),
        integrator_loop::definition_lookup(
            &protocol_events,
            &protocol_snapshots,
            &protocol_definitions,
            &protocol_publications,
        ),
        &host_delivery,
        &agentic_system,
        clock.clone(),
    );
    let progress_notifier = Arc::new(ProtocolProgressNotifier::new());
    let protocol_agent_status_port = protocol_agent_status::port();
    let projections = protocol_publisher::EngineProjections {
        memory: protocol_publisher::memory_recorder(memory_writer, &protocol_events),
        progress: progress_notifier.clone(),
        events: protocol_events.clone(),
        snapshots: protocol_snapshots,
        metrics: metrics_recorder.clone(),
        deliveries: host_delivery.ledger.clone(),
        agent_status: protocol_agent_status_port.clone(),
        attention: attention_recovery.clone(),
    };
    let protocol_stream = protocol_publisher::stream(projections, event_publisher);
    let memory_reader = authorization.protect_runtime(memory_reader, protocol_stream.clone());

    let council_operations::CouncilOperations {
        deliberate,
        orchestrate,
        run_council_decision,
        step_handler: protocol_step_handler,
    } = council_operations::wire(council_dependencies::CouncilDependencies {
        clock: clock.clone(),
        councils: council_registry.clone(),
        resolver: agent_resolver.clone(),
        validators,
        scoring,
        repository: repository.clone(),
        messaging: messaging.clone(),
        statistics: statistics.clone(),
        metrics: metrics_recorder.clone(),
        executor,
        contracts: contract_registry.clone(),
    });
    let resolve_protocol_definition =
        protocol_definitions::resolver(&protocol_definitions, &protocol_publications);
    let integrator_loop = integrator_loop::wire(
        attention_recovery,
        &host_delivery,
        protocol_events.clone(),
        protocol_stream.clone(),
        resolve_protocol_definition.clone(),
        progress_notifier.clone(),
        clock.clone(),
    );
    let protocol_openings::ProtocolOpenings {
        start_protocol,
        start_published_protocol,
        prepare_protocol_children,
        accept_child_completion,
    } = protocol_openings::wire(
        &resolve_protocol_definition,
        &protocol_definitions,
        &protocol_publications,
        &protocol_stream,
        &clock,
        memory_reader.clone(),
    );
    let budget_operations = budget_operations::BudgetOperations::new(
        budget_ledger,
        protocol_publications.clone(),
        protocol_stream.clone(),
        clock.clone(),
        memory_reader.clone(),
        resolve_protocol_definition.clone(),
        service_config.max_parallel,
    );
    let run_protocol = Arc::new(
        RunProtocolUseCase::new(
            protocol_definitions.clone(),
            protocol_stream.clone(),
            protocol_step_handler.clone(),
            clock.clone(),
        )
        .with_metrics(metrics_recorder.clone())
        .with_max_parallel_ceiling(service_config.max_parallel)
        .with_child_orchestrator(prepare_protocol_children.clone()),
    );
    let recover_protocol_children = Arc::new(children_recovery::wire(
        children_recovery::ChildrenRecoveryDependencies {
            events: protocol_events.clone(),
            cursors: protocol_cursors.clone(),
            stream: protocol_stream.clone(),
            prepare: prepare_protocol_children.clone(),
            accept: accept_child_completion.clone(),
            clock: clock.clone(),
            continuation: authorization_continuation.clone(),
        },
    )?);
    protocol_operations::recover_to_head(&recover_protocol_children).await?;
    let run_protocol_step = Arc::new(
        RunProtocolStepUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            protocol_step_handler,
            clock.clone(),
        )
        .with_max_parallel_ceiling(service_config.max_parallel)
        .with_child_orchestrator(prepare_protocol_children),
    );
    // Delegated hosts claim through the same use case as embedded execution.
    let claim_protocol_step = Arc::new(
        StartProtocolStepUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            clock.clone(),
        )
        .with_max_parallel_ceiling(service_config.max_parallel),
    );
    let lifecycle = ProtocolLifecycleControls::wire(
        resolve_protocol_definition.clone(),
        protocol_stream.clone(),
        clock.clone(),
        execution_receipts.clone(),
        authorization.authorize.clone(),
        protocol_publications.clone(),
    );

    let registry_operations = registry_operations::RegistryOperations {
        clock: clock.clone(),
        factory: agent_factory.clone(),
        agents: agent_registry.clone(),
        resolver: agent_resolver.clone(),
        councils: council_registry.clone(),
        repository: repository.clone(),
    };

    let auto_dispatch = Arc::new(AutoDispatchService::new(
        deliberate.clone(),
        "Investigate the incoming trigger event.",
    )?);

    registry_operations.seed(contract_registry.as_ref()).await?;

    // Auto-dispatch completes subscriber wiring.
    let nats_subscriber = nats_subscriber_factory.map(|factory| factory(auto_dispatch.clone()));
    let nats_protocol_recovery =
        protocol_recovery_factory.map(|factory| factory(recover_protocol_children.clone()));

    let worker_daemon = protocol_workers::wire(protocol_workers::ProtocolWorkerDependencies {
        index: protocol_index.clone(),
        stream: protocol_stream.clone(),
        definitions: resolve_protocol_definition.clone(),
        deadlines: lifecycle.enforce_deadlines.clone(),
        start_step: claim_protocol_step.clone(),
        receipts: execution_receipts.clone(),
        clock: clock.clone(),
        artifacts: artifacts.clone(),
        budgets: budget_operations.service(),
        budgeted_claim: budget_operations.claim(),
        budget_planner: None,
        authorize: authorization.authorize.clone(),
        continuation: authorization_continuation.clone(),
    })?;

    let mut grpc_builder = authorization
        .apply(made_adapters::grpc::MadeGrpcService::builder())
        .deliberate(deliberate)
        .orchestrate(orchestrate)
        .run_council_decision(run_council_decision)
        .run_protocol(run_protocol)
        .start_protocol(start_protocol)
        .start_published_protocol(start_published_protocol)
        .run_protocol_step(run_protocol_step)
        .accept_child_completion(accept_child_completion)
        .recover_protocol_children(recover_protocol_children)
        .claim_protocol_step(claim_protocol_step)
        .protocol_definitions(protocol_definitions.clone())
        .resolve_protocol_definition(resolve_protocol_definition.clone())
        .contract_registry(contract_registry.clone())
        .auto_dispatch(auto_dispatch)
        .statistics(statistics.clone())
        // Status and `/metrics` read the same registry use cases write.
        .observability(metrics_recorder.clone())
        .service_version(env!("CARGO_PKG_VERSION"))
        .council_journal(Arc::new(made_app::services::CouncilJournalService::new(
            council_journal,
            clock.clone(),
        )))
        .clock(clock.clone())
        .max_parallel_ceiling(service_config.max_parallel);
    let protocol_agent_status = protocol_agent_status::service(
        protocol_agent_status_port.clone(),
        &clock,
        &protocol_stream,
    );
    grpc_builder = grpc_builder.protocol_agent_status(protocol_agent_status.clone());
    grpc_builder = lifecycle.apply_to(grpc_builder);
    grpc_builder = registry_operations.wire(grpc_builder);
    grpc_builder = budget_operations.wire(grpc_builder);
    grpc_builder = protocol_queries::wire(
        grpc_builder,
        resolve_protocol_definition.clone(),
        protocol_stream.clone(),
        protocol_index,
        protocol_events,
        protocol_cursors,
        progress_notifier,
        protocol_agent_status_port,
        protocol_publications.clone(),
    )?;
    grpc_builder = execution_receipts::wire(
        grpc_builder,
        resolve_protocol_definition.clone(),
        protocol_stream.clone(),
        execution_receipts,
        clock.clone(),
        artifacts.clone(),
        budget_operations.service(),
    );
    grpc_builder = protocol_operations::wire(
        grpc_builder,
        resolve_protocol_definition,
        &protocol_stream,
        &clock,
        authorization_continuation,
        renewal_authorization,
        &host_delivery.ledger,
        protocol_agent_status,
    );
    grpc_builder = agentic_system::apply_to(
        grpc_builder,
        &agentic_system,
        agentic_system::AgenticSystemDependencies {
            publications: protocol_publications.clone(),
            definitions: protocol_definitions,
            stream: protocol_stream,
            clock,
            bindings: host_delivery.bindings.clone(),
        },
    );
    grpc_builder = integrator_loop::apply_to(grpc_builder, &integrator_loop);
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

    runtime_log::wired(&service_config, &supported_agent_kinds);

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
        nats_protocol_recovery,
        worker_daemon,
        health_state,
        host_delivery,
        agentic_system,
        integrator_loop,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use made_adapters::sqlite::SqliteAuthorizationPolicyStore;
    use made_app::authorization::AuthorizationPolicyAdministrationService;
    use made_core::value_objects::{
        AuthenticatedPrincipal, AuthenticationMethod, AuthorizationPolicyId, PrincipalId,
        PrincipalKind,
    };
    use made_proto::runtime_v1 as runtime_pb;
    use tonic::{transport::Server, Request, Response, Status};

    // One lock prevents concurrent tests racing over process-wide MADE_* variables.
    static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    async fn configure_test_authorization() -> tempfile::TempDir {
        let scratch = std::env::current_dir().unwrap().join("tmp");
        std::fs::create_dir_all(&scratch).unwrap();
        let directory = tempfile::tempdir_in(scratch).unwrap();
        let store_path = directory.path().join("made.sqlite3");
        let principals_path = directory.path().join("principals.json");
        std::fs::write(
            &principals_path,
            serde_json::to_vec(&serde_json::json!([{
                "certificate_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "principal_id": "compose-test-owner",
                "principal_kind": "trusted_host"
            }]))
            .unwrap(),
        )
        .unwrap();
        let policy_id = AuthorizationPolicyId::new("compose-test-policy").unwrap();
        let owner = AuthenticatedPrincipal::new(
            PrincipalId::new("compose-test-owner").unwrap(),
            PrincipalKind::TrustedHost,
            AuthenticationMethod::MutualTls,
        )
        .unwrap();
        AuthorizationPolicyAdministrationService::new(
            policy_id,
            Arc::new(SqliteAuthorizationPolicyStore::open(&store_path).unwrap()),
            Arc::new(SystemClock::new()),
        )
        .open(owner, Vec::new())
        .await
        .unwrap();

        std::env::set_var("MADE_PROTOCOL_STORE_PATH", &store_path);
        std::env::set_var("MADE_GRPC_TLS_MODE", "mutual");
        std::env::set_var("MADE_GRPC_TLS_CERT_PATH", "test-server-cert.pem");
        std::env::set_var("MADE_GRPC_TLS_KEY_PATH", "test-server-key.pem");
        std::env::set_var("MADE_GRPC_TLS_CLIENT_CA_PATH", "test-client-ca.pem");
        std::env::set_var("MADE_AUTH_POLICY_ID", "compose-test-policy");
        std::env::set_var("MADE_PROTOCOL_STORE_ID", "compose-test-store");
        std::env::set_var("MADE_PROTOCOL_SEARCH_CURSOR_HMAC_KEY", "5a".repeat(32));
        std::env::set_var("MADE_AUTH_MTLS_PRINCIPALS_PATH", principals_path);
        directory
    }

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
        let _authorization = configure_test_authorization().await;

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
        let _authorization = configure_test_authorization().await;

        let app = compose().await.expect("compose should succeed");
        assert!(!app.service_config.nats_enabled);
        assert!(app.nats_subscriber.is_none());

        let _ = shutdown.send(());
        std::env::remove_var("MADE_NATS_ENABLED");
        std::env::remove_var("MADE_EXECUTOR_KIND");
        std::env::remove_var("MADE_RUNTIME_GRPC_ENDPOINT");
    }
}
