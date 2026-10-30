//! In-process gRPC fixture for Epic 9 (and any later RPC slice that
//! wants to drive the real MADE over a Tonic channel).
//!
//! The fixture wires the full composition graph in-memory — NATS
//! disabled, Postgres disabled, executor=noop — then mounts the
//! resulting `MadeGrpcService` on a `tokio::net::TcpListener`
//! bound to `127.0.0.1:0`. Tests get back a `tonic::transport::Channel`
//! plus handles to the in-memory contract / council / agent
//! registries, so they can seed state through the registry ports
//! and assert through the RPC surface in the same test.
//!
//! Why a manual wire instead of `made::compose::compose`?
//! `compose` reads from process env and would force every test that
//! uses this fixture to serialize through a shared lock — concurrent
//! `cargo test --workspace` would either deadlock or fight over env
//! vars. The fixture instead constructs the use-case graph with
//! explicit deps so each test gets a fresh, isolated registry set.

use std::sync::Arc;
use std::time::Duration;

use made_adapters::agents::DispatchingAgentFactory;
use made_adapters::clock::SystemClock;
use made_adapters::grpc::MadeGrpcService;
use made_adapters::memory::{
    InMemoryAgentRegistry, InMemoryBudgetLedgerStore, InMemoryContractRegistry,
    InMemoryCouncilRegistry, InMemoryDeliberationRepository, InMemoryProtocolAgentStatus,
    InMemoryProtocolDefinitionPublications, InMemoryProtocolDefinitionRepository,
    InMemoryProtocolEventCursor, InMemoryProtocolEventStore, InMemoryStatistics,
};
use made_adapters::metrics::PrometheusMetricsRecorder;
use made_adapters::noop::{NoopExecutor, NoopMessaging, NoopProtocolEvidenceSource};
use made_adapters::progress::ProtocolProgressNotifier;
use made_adapters::protocol::{
    DeliberatingProtocolStepHandler, ProtocolFanoutMetricsSubscriber, ProtocolMetricsSubscriber,
};
use made_adapters::scoring::UniformScoring;
use made_adapters::validators::{
    AllowedStringValuesValidator, ContentNonEmptyValidator, JsonObjectOutputValidator,
    JsonSchemaValidator, RequiredFieldsValidator,
};
use made_app::artifacts::ArtifactService;
use made_app::authorization::AuthorizedMemoryReader;
use made_app::budgets::{
    BudgetLedgerService, BudgetedStepClaimUseCase, StartBudgetedProtocolUseCase,
};
use made_app::services::{
    AutoDispatchService, ProtocolEventFanout, SessionMemoryRecorder, SessionStream,
};
use made_app::usecases::{
    AcceptChildCompletionUseCase, AcknowledgeProtocolAgentInterventionUseCase,
    ApplyProtocolTransitionUseCase, ApproveProtocolGuardUseCase, AssertProtocolReasonUseCase,
    BindProtocolParticipantsUseCase, CancelProtocolUseCase, CloseProtocolInterventionUseCase,
    CollectProtocolEvidenceUseCase, CompleteProtocolStepUseCase, CreateCouncilUseCase,
    DeferProtocolGuardUseCase, DeleteCouncilUseCase, DeliberateUseCase,
    DiffProtocolDefinitionsUseCase, EnforceProtocolDeadlinesUseCase, GenerateProtocolReportUseCase,
    GetDeliberationUseCase, GetProtocolInstanceUseCase, GetProtocolInterventionUseCase,
    GetProtocolTranscriptUseCase, ListCouncilsUseCase, ListProtocolInstancesUseCase,
    ListProtocolInterventionsUseCase, OrchestrateUseCase, PauseProtocolUseCase,
    PrepareProtocolChildrenUseCase, PrepareProtocolParticipantsUseCase, ProtocolAgentStatusService,
    ProtocolSearchCursorCodec, ProtocolSearchCursorKey, ProtocolSearchCursorNamespace,
    PublishProtocolDefinitionUseCase, PullProtocolAgentInterventionsUseCase,
    PullProtocolEventsUseCase, ReadProtocolEventsUseCase, RecoverProtocolChildrenUseCase,
    RegisterAgentUseCase, RequestProtocolInterventionUseCase, ResolveProtocolDefinitionUseCase,
    RespondToProtocolInterventionUseCase, ResumeProtocolUseCase, RunCouncilDecisionUseCase,
    RunProtocolStepUseCase, RunProtocolUseCase, SearchProtocolInstancesUseCase,
    StartProtocolStepUseCase, StartProtocolUseCase, StartPublishedProtocolUseCase,
    StreamProtocolUseCase, UnregisterAgentUseCase, VerifyProtocolJournalUseCase,
};
use made_core::ports::{
    AgentRegistryPort, AgentResolverPort, ContractRegistryPort, CouncilRegistryPort,
    ProtocolDefinitionPublicationPort, ProtocolDefinitionRepositoryPort, ProtocolInstanceIndexPort,
    ProtocolStepHandlerPort, ValidatorPort,
};
use made_core::value_objects::ProtocolEventConsumer;
use tokio::sync::oneshot;
use tonic::transport::{Certificate, Channel, Endpoint, Identity, Server, ServerTlsConfig};

use crate::grpc_fixture_authorization::fixture_authorization;

pub use crate::grpc_fixture_wiring::GrpcFixtureWiring;
pub use crate::tls_server_setup::TlsServerSetup;

fn search_protocols(
    index: Arc<dyn ProtocolInstanceIndexPort>,
    stream: Arc<SessionStream>,
) -> Arc<SearchProtocolInstancesUseCase> {
    Arc::new(SearchProtocolInstancesUseCase::new(
        index,
        stream,
        ProtocolSearchCursorCodec::new(
            ProtocolSearchCursorKey::new([0x5a; 32]),
            ProtocolSearchCursorNamespace::new("grpc-fixture-store", "grpc-fixture")
                .expect("fixture search cursor namespace should be valid"),
        ),
    ))
}

/// Handles a test needs to drive the in-process made:
/// the gRPC channel for issuing RPCs and the registries for seeding
/// state without going through CRUD calls. `addr` is the bound
/// `127.0.0.1:<ephemeral>` so callers can build their own TLS
/// channels against the same listener.
pub struct GrpcFixture {
    pub channel: Channel,
    pub addr: std::net::SocketAddr,
    pub contracts: Arc<dyn ContractRegistryPort>,
    pub councils: Arc<dyn CouncilRegistryPort>,
    pub agents: Arc<dyn AgentRegistryPort>,
    pub agent_resolver: Arc<dyn AgentResolverPort>,
    /// The published catalogue, so a test can seed it directly. There
    /// is no publish RPC yet, and a server whose catalogue is filled
    /// by another route is exactly the situation this models.
    pub protocol_publications: Arc<dyn ProtocolDefinitionPublicationPort>,
    /// Held for the lifetime of the fixture; drop kills the server.
    _shutdown: oneshot::Sender<()>,
}

impl GrpcFixture {
    /// Wire and start a fresh fixture. Returns once the server is
    /// accepting connections.
    pub async fn start() -> Self {
        Self::start_with(GrpcFixtureWiring::new()).await
    }

    /// A second server over a store another one already wrote to.
    ///
    /// Everything else is fresh, the definition repository included, so
    /// this is the restart boundary in one process: the streams are
    /// there and the definitions that produced them are not.
    pub async fn start_over(protocol_store: Arc<InMemoryProtocolEventStore>) -> Self {
        Self::start_with(GrpcFixtureWiring::new().with_protocol_store(protocol_store)).await
    }

    /// Wire and start a fixture, with the adapters the caller has an
    /// opinion about replaced.
    ///
    /// Everything left unset is what `compose::compose` picks, so this
    /// is the same server the other tests get. The parity session is
    /// why the seam exists: it has to run the **same** step handler,
    /// evidence source and clock here and in the in-process edition,
    /// and it must get there without changing what production composes.
    #[allow(clippy::too_many_lines)] // wiring graph mirrors `compose::compose`; splitting fragments the dep order
    pub async fn start_with(wiring: GrpcFixtureWiring) -> Self {
        let protocol_store = wiring.protocol_store();
        let protocol_index = wiring.protocol_index();
        let clock = wiring.clock();
        let memory = wiring.memory();
        let validators: Vec<Arc<dyn ValidatorPort>> = vec![
            Arc::new(ContentNonEmptyValidator::new()),
            Arc::new(JsonObjectOutputValidator::new()),
            Arc::new(RequiredFieldsValidator::new()),
            Arc::new(AllowedStringValuesValidator::new()),
            Arc::new(JsonSchemaValidator::new()),
        ];
        let scoring = Arc::new(UniformScoring::new());
        let executor = Arc::new(NoopExecutor::new());
        let council_journal = wiring.council_journal();
        let messaging = Arc::new(
            made_adapters::council_journal_messaging::CouncilJournalMessaging::new(
                council_journal.clone(),
            ),
        );
        let statistics = Arc::new(InMemoryStatistics::new());
        let repository = Arc::new(InMemoryDeliberationRepository::new());
        let council_registry: Arc<dyn CouncilRegistryPort> =
            Arc::new(InMemoryCouncilRegistry::new());
        let contract_registry: Arc<dyn ContractRegistryPort> =
            Arc::new(InMemoryContractRegistry::new());
        let protocol_definitions: Arc<dyn ProtocolDefinitionRepositoryPort> =
            Arc::new(InMemoryProtocolDefinitionRepository::new());
        // One adapter, both directions: what a session records is what
        // the next session in that scope is told. The composition
        // root's own wiring: memory is a subscriber of the stream, and
        // the one the fixture wires by default forgets.
        let (memory_writer, memory_reader) = memory;
        let metrics = Arc::new(
            PrometheusMetricsRecorder::new().expect("fixture metrics registry should build"),
        );
        let progress_notifier = Arc::new(ProtocolProgressNotifier::new());
        let agent_status = Arc::new(InMemoryProtocolAgentStatus::new());
        let deliveries: Arc<dyn made_core::ports::HostDeliveryLedgerPort> =
            Arc::new(made_adapters::memory::InMemoryHostDeliveryLedger::new());
        let integrator_bindings: Arc<dyn made_core::ports::IntegratorBindingPort> =
            Arc::new(made_adapters::memory::InMemoryIntegratorBindings::new());
        let agentic_system_repository: Arc<dyn made_core::ports::AgenticSystemRepositoryPort> =
            Arc::new(made_adapters::memory::InMemoryAgenticSystemRepository::new());
        let agentic_system_executions: Arc<dyn made_core::ports::AgenticSystemExecutionStorePort> =
            Arc::new(made_adapters::memory::InMemoryAgenticSystemExecutions::new());
        let protocol_cursors = Arc::new(InMemoryProtocolEventCursor::new());
        let protocol_publications: Arc<dyn ProtocolDefinitionPublicationPort> =
            Arc::new(InMemoryProtocolDefinitionPublications::new());
        // The projection the loop reads, built before the fanout that
        // wakes it: one walk of the feed, two sides. A fixture without
        // it would answer every await with an empty queue and look
        // exactly like a healthy one.
        let attention_recovery = Arc::new(made_app::services::attention::AttentionRecovery::new(
            integrator_bindings.clone(),
            Arc::new(
                made_app::services::attention::AttentionAudienceResolver::new(
                    agentic_system_executions.clone(),
                    agentic_system_repository.clone(),
                ),
            ),
            Arc::new(made_app::services::attention::AttentionProjector::new(
                protocol_store.clone(),
                protocol_cursors.clone(),
                deliveries.clone(),
                Arc::new(made_adapters::activation::NoHostActivation::new()),
                Arc::new(
                    made_app::services::attention::SessionDefinitionLookup::over(
                        protocol_store.clone(),
                        wiring.protocol_snapshots(),
                        protocol_definitions.clone(),
                        protocol_publications.clone(),
                    ),
                ),
                clock.clone(),
            )),
        ));
        let protocol_stream = Arc::new(SessionStream::new_authorized(
            protocol_store.clone(),
            wiring.protocol_snapshots(),
            Arc::new(ProtocolEventFanout::new(vec![
                progress_notifier.clone(),
                Arc::new(SessionMemoryRecorder::new(
                    memory_writer,
                    protocol_store.clone(),
                )),
                Arc::new(ProtocolMetricsSubscriber::new(metrics.clone())),
                Arc::new(ProtocolFanoutMetricsSubscriber::new(
                    protocol_store.clone(),
                    metrics.clone(),
                )),
                Arc::new(made_app::services::InterventionDeliverySubscriber::new(
                    deliveries.clone(),
                    agent_status.clone(),
                )),
                Arc::new(made_app::services::attention::AttentionSubscriber::new(
                    attention_recovery.clone(),
                )),
            ])),
        ));
        let fixture_authorization = fixture_authorization(clock.clone()).await;
        let memory_reader = Arc::new(AuthorizedMemoryReader::new(
            memory_reader,
            fixture_authorization.authorize.clone(),
            protocol_stream.clone(),
        ));
        let budgets =
            BudgetLedgerService::new(Arc::new(InMemoryBudgetLedgerStore::new()), clock.clone());
        let resolve_protocol_definition = Arc::new(ResolveProtocolDefinitionUseCase::new(
            protocol_definitions.clone(),
            protocol_publications.clone(),
        ));
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
            metrics.clone(),
            "made-tests",
        ));
        let protocol_step_handler: Arc<dyn ProtocolStepHandlerPort> = wiring.step_handler(|| {
            Arc::new(DeliberatingProtocolStepHandler::new(deliberate.clone()))
                as Arc<dyn ProtocolStepHandlerPort>
        });
        let orchestrate = Arc::new(OrchestrateUseCase::new(
            deliberate.clone(),
            executor,
            messaging.clone(),
            clock.clone(),
            statistics.clone(),
            "made-tests",
        ));
        let run_council_decision = Arc::new(RunCouncilDecisionUseCase::new(
            contract_registry.clone(),
            council_registry.clone(),
            deliberate.clone(),
            repository.clone(),
        ));
        let child_orchestrator = Arc::new(PrepareProtocolChildrenUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_publications.clone(),
            protocol_stream.clone(),
            clock.clone(),
            memory_reader.clone(),
        ));
        let accept_child_completion = Arc::new(AcceptChildCompletionUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_publications.clone(),
            protocol_stream.clone(),
            clock.clone(),
        ));
        let recover_protocol_children = Arc::new(RecoverProtocolChildrenUseCase::new(
            protocol_store.clone(),
            protocol_cursors.clone(),
            protocol_stream.clone(),
            child_orchestrator.clone(),
            accept_child_completion.clone(),
            clock.clone(),
            ProtocolEventConsumer::new("made.children.recovery.v1")
                .expect("fixture recovery consumer should be valid"),
        ));
        let run_protocol = Arc::new(
            RunProtocolUseCase::new(
                protocol_definitions.clone(),
                protocol_stream.clone(),
                protocol_step_handler.clone(),
                clock.clone(),
            )
            .with_child_orchestrator(child_orchestrator.clone()),
        );
        let start_protocol = Arc::new(StartProtocolUseCase::new(
            protocol_definitions.clone(),
            protocol_stream.clone(),
            clock.clone(),
            memory_reader.clone(),
        ));
        let start_published_protocol = Arc::new(StartPublishedProtocolUseCase::new(
            protocol_publications.clone(),
            protocol_stream.clone(),
            clock.clone(),
            memory_reader.clone(),
        ));
        let start_budgeted_protocol = Arc::new(StartBudgetedProtocolUseCase::new(
            protocol_publications.clone(),
            protocol_stream.clone(),
            clock.clone(),
            memory_reader.clone(),
            budgets.clone(),
        ));
        let run_protocol_step = Arc::new(
            RunProtocolStepUseCase::new(
                resolve_protocol_definition.clone(),
                protocol_stream.clone(),
                protocol_step_handler,
                clock.clone(),
            )
            .with_child_orchestrator(child_orchestrator),
        );
        // The delegated-host protocol reaches the fixture too: the
        // parity tests drive claim and complete over these very RPCs,
        // and a fixture missing them would prove the tools agree on a
        // server nobody runs.
        let claim_protocol_step = Arc::new(StartProtocolStepUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            clock.clone(),
        ));
        let budgeted_step_claim = Arc::new(BudgetedStepClaimUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            clock.clone(),
            budgets.clone(),
        ));
        let complete_protocol_step = Arc::new(CompleteProtocolStepUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            clock.clone(),
        ));
        let renew_protocol_step_lease = Arc::new(
            made_app::workers::RenewProtocolStepLeaseUseCase::new(
                protocol_stream.clone(),
                resolve_protocol_definition.clone(),
                clock.clone(),
            )
            .with_reauthorization(fixture_authorization.authorize.clone()),
        );
        let apply_protocol_transition = Arc::new(ApplyProtocolTransitionUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            clock.clone(),
        ));
        let pause_protocol = Arc::new(PauseProtocolUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            clock.clone(),
        ));
        let resume_protocol = Arc::new(ResumeProtocolUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            clock.clone(),
        ));
        let cancel_protocol = Arc::new(CancelProtocolUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            clock.clone(),
        ));
        let enforce_protocol_deadlines = Arc::new(EnforceProtocolDeadlinesUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            clock.clone(),
        ));
        let assert_protocol_reason = Arc::new(AssertProtocolReasonUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            clock.clone(),
        ));
        let approve_protocol_guard = Arc::new(ApproveProtocolGuardUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            clock.clone(),
        ));
        let defer_protocol_guard = Arc::new(DeferProtocolGuardUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            clock.clone(),
        ));
        let request_protocol_intervention = Arc::new(RequestProtocolInterventionUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            clock.clone(),
        ));
        let respond_to_protocol_intervention = Arc::new(
            RespondToProtocolInterventionUseCase::new(
                resolve_protocol_definition.clone(),
                protocol_stream.clone(),
                clock.clone(),
            )
            .with_delivery_ledger(deliveries.clone()),
        );
        let close_protocol_intervention = Arc::new(CloseProtocolInterventionUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            clock.clone(),
        ));
        // No evidence source ships with the server, so this answers
        // NOT_FOUND until an operator wires one. Failing plainly beats
        // a missing method or an invented answer.
        let collect_protocol_evidence = Arc::new(CollectProtocolEvidenceUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            wiring.evidence_source(),
            clock.clone(),
        ));
        let agent_status_service = Arc::new(
            ProtocolAgentStatusService::new(
                agent_status.clone(),
                wiring.clock(),
                time::Duration::seconds(60),
            )
            .with_journal_claims(protocol_stream.clone()),
        );
        let pull_protocol_agent_interventions =
            Arc::new(PullProtocolAgentInterventionsUseCase::new(
                protocol_stream.clone(),
                agent_status_service.clone(),
                deliveries.clone(),
                clock.clone(),
            ));
        let acknowledge_protocol_agent_intervention =
            Arc::new(AcknowledgeProtocolAgentInterventionUseCase::new(
                resolve_protocol_definition.clone(),
                protocol_stream.clone(),
                deliveries.clone(),
                clock.clone(),
            ));
        let get_protocol_intervention = Arc::new(GetProtocolInterventionUseCase::new(
            protocol_stream.clone(),
            deliveries.clone(),
        ));
        let list_protocol_interventions = Arc::new(ListProtocolInterventionsUseCase::new(
            protocol_stream.clone(),
            deliveries.clone(),
        ));
        let publish_protocol_definition = Arc::new(PublishProtocolDefinitionUseCase::new(
            protocol_publications.clone(),
        ));
        let diff_protocol_definitions = Arc::new(DiffProtocolDefinitionsUseCase::new(
            protocol_publications.clone(),
        ));
        let bind_protocol_participants = Arc::new(BindProtocolParticipantsUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            clock.clone(),
        ));
        let create_council = Arc::new(CreateCouncilUseCase::new(
            clock.clone(),
            council_registry.clone(),
            agent_resolver.clone(),
        ));
        let prepare_protocol_participants = Arc::new(PrepareProtocolParticipantsUseCase::new(
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
        let auto_dispatch = Arc::new(
            AutoDispatchService::new(
                deliberate.clone(),
                "Investigate the incoming trigger event.",
            )
            .expect("auto-dispatch wiring should never fail"),
        );

        let get_protocol_instance =
            Arc::new(GetProtocolInstanceUseCase::new(protocol_stream.clone()));
        let authorization = wiring
            .authorization()
            .unwrap_or_else(|| fixture_authorization.gate.clone());
        authorization.protect_session_stream(&protocol_stream);
        // The agentic-system stores this fixture composes. In memory,
        // like everything else here, and wired through the same
        // launcher a service uses, so the two backends open their
        // protocols the same way.
        let agentic_system = Arc::new(made_adapters::grpc::AgenticSystemOperations::new(
            agentic_system_repository,
            Arc::new(made_adapters::memory::InMemoryAgenticSystemPublications::new()),
            agentic_system_executions,
            Arc::new(made_adapters::mermaid::AgenticSystemMermaidDiagram::new()),
            protocol_publications.clone(),
            Arc::new(made_app::usecases::agentic_system::ProtocolLauncher::new(
                start_published_protocol.clone(),
                bind_protocol_participants.clone(),
            )),
            Arc::new(made_app::usecases::agentic_system::Observation::new(
                protocol_stream.clone(),
            )),
            None,
            clock.clone(),
        ));
        // The same five the deployable service composes, over the same
        // projection its subscriber wakes.
        let integrator_loop = Arc::new(made_adapters::grpc::IntegratorLoopOperations::new(
            Arc::new(
                made_app::usecases::integrator::BindProtocolIntegratorUseCase::new(
                    integrator_bindings.clone(),
                    deliveries.clone(),
                    clock.clone(),
                ),
            ),
            Arc::new(
                made_app::usecases::integrator::GetProtocolIntegratorBindingUseCase::new(
                    integrator_bindings.clone(),
                ),
            ),
            Arc::new(
                made_app::usecases::integrator::AwaitIntegratorAttentionUseCase::new(
                    integrator_bindings.clone(),
                    deliveries.clone(),
                    protocol_store.clone(),
                    protocol_stream.clone(),
                    resolve_protocol_definition.clone(),
                    progress_notifier.clone(),
                    clock.clone(),
                )
                .with_recovery(attention_recovery.clone()),
            ),
            Arc::new(
                made_app::usecases::integrator::AcknowledgeIntegratorAttentionUseCase::new(
                    integrator_bindings,
                    deliveries.clone(),
                    clock.clone(),
                ),
            ),
            Arc::new(
                made_app::usecases::integrator::ListAttentionDeliveriesUseCase::new(
                    deliveries.clone(),
                )
                .with_recovery(attention_recovery),
            ),
        ));
        let mut service_builder = MadeGrpcService::builder()
            .agentic_system(agentic_system)
            .record_protocol_host_handoff(Arc::new(
                made_app::workers::RecordProtocolHostHandoffUseCase::new(
                    protocol_stream.clone(),
                    resolve_protocol_definition.clone(),
                    clock.clone(),
                )
                .with_reauthorization(fixture_authorization.authorize.clone()),
            ))
            .inspect_protocol_resume(Arc::new(
                made_app::workers::InspectProtocolResumeUseCase::new(
                    protocol_stream.clone(),
                    wiring.execution_receipts(),
                    clock.clone(),
                ),
            ))
            .plan_protocol_successor(Arc::new(
                made_app::usecases::PlanProtocolSuccessorUseCase::new(
                    resolve_protocol_definition.clone(),
                    protocol_publications.clone(),
                    protocol_stream.clone(),
                    Arc::new(made_app::workers::InspectProtocolResumeUseCase::new(
                        protocol_stream.clone(),
                        wiring.execution_receipts(),
                        clock.clone(),
                    )),
                ),
            ))
            .start_protocol_successor(Arc::new(
                made_app::usecases::StartProtocolSuccessorUseCase::new(
                    resolve_protocol_definition.clone(),
                    protocol_publications.clone(),
                    protocol_stream.clone(),
                    clock.clone(),
                )
                .with_reauthorization(fixture_authorization.authorize.clone()),
            ))
            .authorization(authorization)
            .authorization_administration(fixture_authorization.administration)
            .read_authorization_policy(fixture_authorization.read_policy)
            .read_authorization_decisions(fixture_authorization.read_decisions)
            .continue_accepted_step_claim(Arc::new(
                made_app::authorization::ContinueAcceptedStepClaimUseCase::new(
                    protocol_stream.clone(),
                    fixture_authorization.continuation,
                    clock.clone(),
                ),
            ))
            .deliberate(deliberate)
            .orchestrate(orchestrate)
            .create_council(create_council)
            .delete_council(delete_council)
            .list_councils(list_councils)
            .get_deliberation(get_deliberation)
            .register_agent(register_agent)
            .unregister_agent(unregister_agent)
            .run_council_decision(run_council_decision)
            .run_protocol(run_protocol)
            .start_protocol(start_protocol)
            .start_published_protocol(start_published_protocol)
            .start_budgeted_protocol(start_budgeted_protocol)
            .run_protocol_step(run_protocol_step)
            .accept_child_completion(accept_child_completion)
            .recover_protocol_children(recover_protocol_children)
            .claim_protocol_step(claim_protocol_step)
            .budgeted_step_claim(budgeted_step_claim)
            .budgets(Arc::new(budgets.clone()))
            .complete_protocol_step(complete_protocol_step)
            .renew_protocol_step_lease(renew_protocol_step_lease)
            .apply_protocol_transition(apply_protocol_transition)
            .pause_protocol(pause_protocol)
            .resume_protocol(resume_protocol)
            .cancel_protocol(cancel_protocol)
            .enforce_protocol_deadlines(enforce_protocol_deadlines)
            .approve_protocol_guard(approve_protocol_guard)
            .defer_protocol_guard(defer_protocol_guard)
            .assert_protocol_reason(assert_protocol_reason)
            .request_protocol_intervention(request_protocol_intervention)
            .respond_to_protocol_intervention(respond_to_protocol_intervention)
            .close_protocol_intervention(close_protocol_intervention)
            .pull_protocol_agent_interventions(pull_protocol_agent_interventions)
            .acknowledge_protocol_agent_intervention(acknowledge_protocol_agent_intervention)
            .get_protocol_intervention(get_protocol_intervention)
            .list_protocol_interventions(list_protocol_interventions)
            .integrator_loop(integrator_loop)
            .collect_protocol_evidence(collect_protocol_evidence)
            .publish_protocol_definition(publish_protocol_definition)
            .diff_protocol_definitions(diff_protocol_definitions)
            .protocol_publications(protocol_publications.clone())
            .bind_protocol_participants(bind_protocol_participants)
            .protocol_definitions(protocol_definitions.clone())
            .get_protocol_instance(get_protocol_instance.clone())
            .list_protocol_instances(Arc::new(ListProtocolInstancesUseCase::new(
                protocol_stream.clone(),
            )))
            .search_protocol_instances(search_protocols(protocol_index, protocol_stream.clone()))
            // What the session left behind. The parity session drives
            // all four over these very RPCs, so a fixture missing
            // them would prove the tools agree on a server nobody
            // runs.
            .read_protocol_events(Arc::new(ReadProtocolEventsUseCase::new(
                protocol_store.clone(),
            )))
            .stream_protocol(Arc::new(
                StreamProtocolUseCase::new(protocol_store.clone(), progress_notifier)
                    .with_agent_activity(agent_status.clone()),
            ))
            .pull_protocol_events(Arc::new(PullProtocolEventsUseCase::new(
                protocol_store.clone(),
                protocol_cursors,
            )))
            .verify_protocol_journal(Arc::new(VerifyProtocolJournalUseCase::new(
                protocol_store.clone(),
            )))
            .get_protocol_transcript(Arc::new(GetProtocolTranscriptUseCase::new(
                protocol_store.clone(),
            )))
            .generate_protocol_report(Arc::new(GenerateProtocolReportUseCase::new(
                resolve_protocol_definition.clone(),
                protocol_store,
            )))
            .resolve_protocol_definition(resolve_protocol_definition.clone())
            .prepare_protocol_participants(prepare_protocol_participants)
            .contract_registry(contract_registry.clone())
            .auto_dispatch(auto_dispatch)
            .statistics(statistics.clone())
            .observability(metrics)
            .service_version("made-tests")
            .council_journal(Arc::new(made_app::services::CouncilJournalService::new(
                council_journal,
                wiring.clock(),
            )))
            .clock(wiring.clock());
        let receipts = wiring.execution_receipts();
        let complete_receipt = made_app::workers::CompleteExecutionReceiptUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            receipts.clone(),
            wiring.clock(),
        );
        let complete_receipt = if let Some(store) = wiring.artifact_store() {
            complete_receipt.with_artifacts(Arc::new(ArtifactService::new(store)))
        } else {
            complete_receipt
        }
        .with_budget_ledger(budgets);
        service_builder = service_builder
            .protocol_agent_status(agent_status_service)
            .get_execution_receipt(Arc::new(
                made_app::workers::GetExecutionReceiptUseCase::new(receipts.clone()),
            ))
            .inspect_execution_recovery(Arc::new(
                made_app::workers::InspectExecutionRecoveryUseCase::new(
                    protocol_stream.clone(),
                    receipts,
                ),
            ))
            .complete_execution_receipt(Arc::new(complete_receipt));
        if let Some(store) = wiring.artifact_store() {
            service_builder = service_builder.artifacts(Arc::new(ArtifactService::new(store)));
        }
        let svc = service_builder
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
            addr,
            contracts: contract_registry,
            councils: council_registry,
            agents: agent_registry,
            agent_resolver,
            protocol_publications,
            _shutdown: shutdown_tx,
        }
    }

    /// Variant of [`Self::start`] that serves over TLS. Mirrors the
    /// wiring graph verbatim — the only delta is the
    /// `Server::builder().tls_config(...)` call and the `https://`
    /// `Endpoint` the returned `channel` is built against. Caller-
    /// supplied PEM bytes (typically minted via
    /// `tls_fixture::mint_tls`) feed both the server's identity and,
    /// for `Mutual`, the client-CA trust anchor.
    ///
    /// The returned `channel` is configured with TLS that trusts the
    /// matching CA and (in mutual mode) carries the minted client
    /// identity, so the "happy path" RPC works out of the box. Tests
    /// that need to construct a *different* client (e.g. one missing
    /// the identity, to assert rejection) use `fixture.addr` directly.
    #[allow(clippy::too_many_lines)] // wiring graph mirrors `compose::compose`; splitting fragments the dep order
    pub async fn start_with_tls(setup: TlsServerSetup) -> Self {
        let clock = Arc::new(SystemClock::new());
        let memory = GrpcFixtureWiring::new().memory();
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
        let protocol_definitions: Arc<dyn ProtocolDefinitionRepositoryPort> =
            Arc::new(InMemoryProtocolDefinitionRepository::new());
        let protocol_store = Arc::new(InMemoryProtocolEventStore::new());
        // One adapter, both directions: what a session records is what
        // the next session in that scope is told. The composition
        // root's own wiring: memory is a subscriber of the stream, and
        // the one the fixture wires by default forgets.
        let (memory_writer, memory_reader) = memory;
        let metrics = Arc::new(
            PrometheusMetricsRecorder::new().expect("fixture metrics registry should build"),
        );
        let progress_notifier = Arc::new(ProtocolProgressNotifier::new());
        let agent_status = Arc::new(InMemoryProtocolAgentStatus::new());
        let deliveries: Arc<dyn made_core::ports::HostDeliveryLedgerPort> =
            Arc::new(made_adapters::memory::InMemoryHostDeliveryLedger::new());
        let protocol_stream = Arc::new(SessionStream::new_authorized(
            protocol_store.clone(),
            protocol_store.clone(),
            Arc::new(ProtocolEventFanout::new(vec![
                progress_notifier.clone(),
                Arc::new(SessionMemoryRecorder::new(
                    memory_writer,
                    protocol_store.clone(),
                )),
                Arc::new(ProtocolMetricsSubscriber::new(metrics.clone())),
                Arc::new(ProtocolFanoutMetricsSubscriber::new(
                    protocol_store.clone(),
                    metrics.clone(),
                )),
                Arc::new(made_app::services::InterventionDeliverySubscriber::new(
                    deliveries.clone(),
                    agent_status.clone(),
                )),
            ])),
        ));
        let fixture_authorization = fixture_authorization(clock.clone()).await;
        let memory_reader = Arc::new(AuthorizedMemoryReader::new(
            memory_reader,
            fixture_authorization.authorize.clone(),
            protocol_stream.clone(),
        ));
        let protocol_cursors = Arc::new(InMemoryProtocolEventCursor::new());
        let protocol_publications: Arc<dyn ProtocolDefinitionPublicationPort> =
            Arc::new(InMemoryProtocolDefinitionPublications::new());
        let resolve_protocol_definition = Arc::new(ResolveProtocolDefinitionUseCase::new(
            protocol_definitions.clone(),
            protocol_publications.clone(),
        ));
        let agent_registry = Arc::new(InMemoryAgentRegistry::new());
        let agent_resolver: Arc<dyn AgentResolverPort> = agent_registry.clone();
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
            metrics.clone(),
            "made-tests",
        ));
        let protocol_step_handler: Arc<dyn ProtocolStepHandlerPort> =
            Arc::new(DeliberatingProtocolStepHandler::new(deliberate.clone()));
        let orchestrate = Arc::new(OrchestrateUseCase::new(
            deliberate.clone(),
            executor,
            messaging.clone(),
            clock.clone(),
            statistics.clone(),
            "made-tests",
        ));
        let run_council_decision = Arc::new(RunCouncilDecisionUseCase::new(
            contract_registry.clone(),
            council_registry.clone(),
            deliberate.clone(),
            repository.clone(),
        ));
        let child_orchestrator = Arc::new(PrepareProtocolChildrenUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_publications.clone(),
            protocol_stream.clone(),
            clock.clone(),
            memory_reader.clone(),
        ));
        let accept_child_completion = Arc::new(AcceptChildCompletionUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_publications.clone(),
            protocol_stream.clone(),
            clock.clone(),
        ));
        let recover_protocol_children = Arc::new(RecoverProtocolChildrenUseCase::new(
            protocol_store.clone(),
            protocol_cursors.clone(),
            protocol_stream.clone(),
            child_orchestrator.clone(),
            accept_child_completion.clone(),
            clock.clone(),
            ProtocolEventConsumer::new("made.children.recovery.v1")
                .expect("fixture recovery consumer should be valid"),
        ));
        let run_protocol = Arc::new(
            RunProtocolUseCase::new(
                protocol_definitions.clone(),
                protocol_stream.clone(),
                protocol_step_handler.clone(),
                clock.clone(),
            )
            .with_child_orchestrator(child_orchestrator.clone()),
        );
        let start_protocol = Arc::new(StartProtocolUseCase::new(
            protocol_definitions.clone(),
            protocol_stream.clone(),
            clock.clone(),
            memory_reader.clone(),
        ));
        let start_published_protocol = Arc::new(StartPublishedProtocolUseCase::new(
            protocol_publications.clone(),
            protocol_stream.clone(),
            clock.clone(),
            memory_reader.clone(),
        ));
        let run_protocol_step = Arc::new(
            RunProtocolStepUseCase::new(
                resolve_protocol_definition.clone(),
                protocol_stream.clone(),
                protocol_step_handler,
                clock.clone(),
            )
            .with_child_orchestrator(child_orchestrator),
        );
        // The delegated-host protocol reaches the fixture too: the
        // parity tests drive claim and complete over these very RPCs,
        // and a fixture missing them would prove the tools agree on a
        // server nobody runs.
        let claim_protocol_step = Arc::new(StartProtocolStepUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            clock.clone(),
        ));
        let complete_protocol_step = Arc::new(CompleteProtocolStepUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            clock.clone(),
        ));
        let renew_protocol_step_lease = Arc::new(
            made_app::workers::RenewProtocolStepLeaseUseCase::new(
                protocol_stream.clone(),
                resolve_protocol_definition.clone(),
                clock.clone(),
            )
            .with_reauthorization(fixture_authorization.authorize.clone()),
        );
        let apply_protocol_transition = Arc::new(ApplyProtocolTransitionUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            clock.clone(),
        ));
        let pause_protocol = Arc::new(PauseProtocolUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            clock.clone(),
        ));
        let resume_protocol = Arc::new(ResumeProtocolUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            clock.clone(),
        ));
        let cancel_protocol = Arc::new(CancelProtocolUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            clock.clone(),
        ));
        let enforce_protocol_deadlines = Arc::new(EnforceProtocolDeadlinesUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            clock.clone(),
        ));
        let assert_protocol_reason = Arc::new(AssertProtocolReasonUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            clock.clone(),
        ));
        let approve_protocol_guard = Arc::new(ApproveProtocolGuardUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            clock.clone(),
        ));
        let defer_protocol_guard = Arc::new(DeferProtocolGuardUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            clock.clone(),
        ));
        let request_protocol_intervention = Arc::new(RequestProtocolInterventionUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            clock.clone(),
        ));
        let respond_to_protocol_intervention = Arc::new(
            RespondToProtocolInterventionUseCase::new(
                resolve_protocol_definition.clone(),
                protocol_stream.clone(),
                clock.clone(),
            )
            .with_delivery_ledger(deliveries.clone()),
        );
        let close_protocol_intervention = Arc::new(CloseProtocolInterventionUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            clock.clone(),
        ));
        // No evidence source ships with the server, so this answers
        // NOT_FOUND until an operator wires one. Failing plainly beats
        // a missing method or an invented answer.
        let collect_protocol_evidence = Arc::new(CollectProtocolEvidenceUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            Arc::new(NoopProtocolEvidenceSource::new()),
            clock.clone(),
        ));
        let agent_status_service = Arc::new(
            ProtocolAgentStatusService::new(
                agent_status.clone(),
                clock.clone(),
                time::Duration::seconds(60),
            )
            .with_journal_claims(protocol_stream.clone()),
        );
        let pull_protocol_agent_interventions =
            Arc::new(PullProtocolAgentInterventionsUseCase::new(
                protocol_stream.clone(),
                agent_status_service.clone(),
                deliveries.clone(),
                clock.clone(),
            ));
        let acknowledge_protocol_agent_intervention =
            Arc::new(AcknowledgeProtocolAgentInterventionUseCase::new(
                resolve_protocol_definition.clone(),
                protocol_stream.clone(),
                deliveries.clone(),
                clock.clone(),
            ));
        let get_protocol_intervention = Arc::new(GetProtocolInterventionUseCase::new(
            protocol_stream.clone(),
            deliveries.clone(),
        ));
        let list_protocol_interventions = Arc::new(ListProtocolInterventionsUseCase::new(
            protocol_stream.clone(),
            deliveries.clone(),
        ));
        let publish_protocol_definition = Arc::new(PublishProtocolDefinitionUseCase::new(
            protocol_publications.clone(),
        ));
        let diff_protocol_definitions = Arc::new(DiffProtocolDefinitionsUseCase::new(
            protocol_publications.clone(),
        ));
        let bind_protocol_participants = Arc::new(BindProtocolParticipantsUseCase::new(
            resolve_protocol_definition.clone(),
            protocol_stream.clone(),
            clock.clone(),
        ));
        let create_council = Arc::new(CreateCouncilUseCase::new(
            clock.clone(),
            council_registry.clone(),
            agent_resolver.clone(),
        ));
        let prepare_protocol_participants = Arc::new(PrepareProtocolParticipantsUseCase::new(
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
        let auto_dispatch = Arc::new(
            AutoDispatchService::new(
                deliberate.clone(),
                "Investigate the incoming trigger event.",
            )
            .expect("auto-dispatch wiring should never fail"),
        );

        let get_protocol_instance =
            Arc::new(GetProtocolInstanceUseCase::new(protocol_stream.clone()));
        fixture_authorization
            .gate
            .protect_session_stream(&protocol_stream);
        let svc = MadeGrpcService::builder()
            .record_protocol_host_handoff(Arc::new(
                made_app::workers::RecordProtocolHostHandoffUseCase::new(
                    protocol_stream.clone(),
                    resolve_protocol_definition.clone(),
                    clock.clone(),
                )
                .with_reauthorization(fixture_authorization.authorize.clone()),
            ))
            .inspect_protocol_resume(Arc::new(
                made_app::workers::InspectProtocolResumeUseCase::new(
                    protocol_stream.clone(),
                    Arc::new(made_adapters::memory::InMemoryExecutionReceiptStore::new()),
                    clock.clone(),
                ),
            ))
            .plan_protocol_successor(Arc::new(
                made_app::usecases::PlanProtocolSuccessorUseCase::new(
                    resolve_protocol_definition.clone(),
                    protocol_publications.clone(),
                    protocol_stream.clone(),
                    Arc::new(made_app::workers::InspectProtocolResumeUseCase::new(
                        protocol_stream.clone(),
                        Arc::new(made_adapters::memory::InMemoryExecutionReceiptStore::new()),
                        clock.clone(),
                    )),
                ),
            ))
            .start_protocol_successor(Arc::new(
                made_app::usecases::StartProtocolSuccessorUseCase::new(
                    resolve_protocol_definition.clone(),
                    protocol_publications.clone(),
                    protocol_stream.clone(),
                    clock.clone(),
                )
                .with_reauthorization(fixture_authorization.authorize.clone()),
            ))
            .authorization(fixture_authorization.gate)
            .authorization_administration(fixture_authorization.administration)
            .read_authorization_policy(fixture_authorization.read_policy)
            .read_authorization_decisions(fixture_authorization.read_decisions)
            .continue_accepted_step_claim(Arc::new(
                made_app::authorization::ContinueAcceptedStepClaimUseCase::new(
                    protocol_stream.clone(),
                    fixture_authorization.continuation,
                    clock.clone(),
                ),
            ))
            .deliberate(deliberate)
            .orchestrate(orchestrate)
            .create_council(create_council)
            .delete_council(delete_council)
            .list_councils(list_councils)
            .get_deliberation(get_deliberation)
            .register_agent(register_agent)
            .unregister_agent(unregister_agent)
            .run_council_decision(run_council_decision)
            .run_protocol(run_protocol)
            .start_protocol(start_protocol)
            .start_published_protocol(start_published_protocol)
            .run_protocol_step(run_protocol_step)
            .accept_child_completion(accept_child_completion)
            .recover_protocol_children(recover_protocol_children)
            .claim_protocol_step(claim_protocol_step)
            .complete_protocol_step(complete_protocol_step)
            .renew_protocol_step_lease(renew_protocol_step_lease)
            .apply_protocol_transition(apply_protocol_transition)
            .pause_protocol(pause_protocol)
            .resume_protocol(resume_protocol)
            .cancel_protocol(cancel_protocol)
            .enforce_protocol_deadlines(enforce_protocol_deadlines)
            .approve_protocol_guard(approve_protocol_guard)
            .defer_protocol_guard(defer_protocol_guard)
            .assert_protocol_reason(assert_protocol_reason)
            .request_protocol_intervention(request_protocol_intervention)
            .respond_to_protocol_intervention(respond_to_protocol_intervention)
            .close_protocol_intervention(close_protocol_intervention)
            .pull_protocol_agent_interventions(pull_protocol_agent_interventions)
            .acknowledge_protocol_agent_intervention(acknowledge_protocol_agent_intervention)
            .get_protocol_intervention(get_protocol_intervention)
            .list_protocol_interventions(list_protocol_interventions)
            .collect_protocol_evidence(collect_protocol_evidence)
            .publish_protocol_definition(publish_protocol_definition)
            .diff_protocol_definitions(diff_protocol_definitions)
            .protocol_publications(protocol_publications.clone())
            .bind_protocol_participants(bind_protocol_participants)
            .protocol_definitions(protocol_definitions.clone())
            .prepare_protocol_participants(prepare_protocol_participants)
            .get_protocol_instance(get_protocol_instance.clone())
            .list_protocol_instances(Arc::new(ListProtocolInstancesUseCase::new(
                protocol_stream.clone(),
            )))
            .search_protocol_instances(search_protocols(
                protocol_store.clone(),
                protocol_stream.clone(),
            ))
            // What the session left behind. The parity session drives
            // all four over these very RPCs, so a fixture missing
            // them would prove the tools agree on a server nobody
            // runs.
            .read_protocol_events(Arc::new(ReadProtocolEventsUseCase::new(
                protocol_store.clone(),
            )))
            .stream_protocol(Arc::new(
                StreamProtocolUseCase::new(protocol_store.clone(), progress_notifier)
                    .with_agent_activity(agent_status.clone()),
            ))
            .pull_protocol_events(Arc::new(PullProtocolEventsUseCase::new(
                protocol_store.clone(),
                protocol_cursors,
            )))
            .verify_protocol_journal(Arc::new(VerifyProtocolJournalUseCase::new(
                protocol_store.clone(),
            )))
            .get_protocol_transcript(Arc::new(GetProtocolTranscriptUseCase::new(
                protocol_store.clone(),
            )))
            .generate_protocol_report(Arc::new(GenerateProtocolReportUseCase::new(
                resolve_protocol_definition.clone(),
                protocol_store,
            )))
            .resolve_protocol_definition(resolve_protocol_definition.clone())
            .contract_registry(contract_registry.clone())
            .auto_dispatch(auto_dispatch)
            .statistics(statistics.clone())
            .observability(metrics)
            .service_version("made-tests")
            .build()
            .expect("grpc service wiring should succeed");

        // Build the server TLS config + remember the materials the
        // client side needs to mint a matching channel.
        let (server_tls, ca_for_client, client_identity_for_channel) = match setup {
            TlsServerSetup::Server { cert, key } => {
                let identity = Identity::from_pem(cert.clone(), key);
                (
                    ServerTlsConfig::new().identity(identity),
                    cert, // happy-path: trust the server's leaf as the anchor
                    None,
                )
            }
            TlsServerSetup::Mutual {
                cert,
                key,
                client_ca,
            } => {
                let identity = Identity::from_pem(cert.clone(), key);
                let server_tls = ServerTlsConfig::new()
                    .identity(identity)
                    .client_ca_root(Certificate::from_pem(client_ca.clone()));
                // In mutual mode the happy-path channel needs both the
                // server-CA (== the server leaf in this fixture, since
                // both leaves share the same self-signed CA) AND a
                // client identity. We accept that the caller wires the
                // client cert+key through their own ClientTlsConfig
                // because the fixture has no access to them here —
                // returning the leaf cert is enough to anchor server
                // verification on the client side; the rejection test
                // builds its own channel without identity.
                (server_tls, cert, Some(client_ca))
            }
        };
        let _ = client_identity_for_channel; // anchor + identity reside with the caller

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("ephemeral bind should succeed");
        let addr = listener.local_addr().expect("local_addr");
        let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        tokio::spawn(async move {
            let _ = Server::builder()
                .tls_config(server_tls)
                .expect("server tls config should be valid")
                .add_service(svc.into_server())
                .serve_with_incoming_shutdown(incoming, async move {
                    let _ = shutdown_rx.await;
                })
                .await;
        });

        // Trust anchor for the happy-path client channel. In server
        // mode this is the server leaf; the integration test will
        // typically rebuild a channel using the CA PEM minted by
        // `mint_tls` (which is the actual trust anchor in this
        // fixture's chain). We connect_lazy() so the first RPC is
        // what triggers the handshake.
        let endpoint = Endpoint::from_shared(format!("https://localhost:{}", addr.port()))
            .expect("endpoint URL")
            .tls_config(
                tonic::transport::ClientTlsConfig::new()
                    .ca_certificate(Certificate::from_pem(ca_for_client))
                    .domain_name("localhost"),
            )
            .expect("client tls config should be valid")
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30));
        let channel = endpoint.connect_lazy();

        GrpcFixture {
            channel,
            addr,
            contracts: contract_registry,
            councils: council_registry,
            agents: agent_registry,
            agent_resolver,
            protocol_publications,
            _shutdown: shutdown_tx,
        }
    }
}
