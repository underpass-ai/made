use crate::embedded_store_openers::{
    open_artifact_store, open_budget_store, protocol_search_cursors_from_env,
};
use crate::engine_projections;
use crate::integrator_loop_ports::{attention_recovery, IntegratorLoopPorts};
use crate::{
    agentic_system_ports::AgenticSystemPorts,
    embedded_authorization_services::EmbeddedAuthorizationServices,
    embedded_council_services::EmbeddedCouncilServices, host_delivery_ports::HostDeliveryPorts,
    EmbeddedMadeBuilder, VERSION,
};
use made_adapters::agents::DispatchingAgentFactory;
use made_adapters::artifacts::LocalArtifactStore;
use made_adapters::progress::ProtocolProgressNotifier;
use made_adapters::sqlite::{
    SqliteAgentRegistry, SqliteBudgetLedgerStore, SqliteContractRegistry, SqliteCouncilJournal,
    SqliteCouncilRegistry, SqliteCouncilStatistics, SqliteCouncilStore,
    SqliteDeliberationRepository, SqliteProtocolStore,
};
use made_api::ApiError;
use made_app::artifacts::ArtifactService;
use made_app::authorization::TrustedHostAuthorizationGate;
use made_app::budgets::BudgetLedgerService;
use made_app::services::attention::SessionDefinitionLookup;
use made_app::services::{ProtocolEventPublisherSubscriber, SessionStream};
use made_app::usecases::{
    GetProtocolInstanceUseCase, GetServiceMetricsUseCase, GetServiceStatusUseCase,
    ListProtocolInstancesUseCase, ProtocolInstancePage, ProtocolProgressSettings,
    ProtocolSearchCursorCodec, PublishProtocolEventsUseCase, ResolveProtocolDefinitionUseCase,
    SearchProtocolInstancesInput, ServiceMetrics, ServiceStatus, StreamProtocolUseCase,
};
use made_core::entities::ProtocolInstance;
use made_core::error::DomainError;
use made_core::ports::{
    AuthorizationPolicyStorePort, ClockPort, ExecutionReceiptStorePort, HostActivationPort,
    HostDeliveryLedgerPort, IntegratorBindingPort, MemoryReaderPort, MemoryWriterPort,
    MetricsRecorderPort, MetricsSnapshotPort, ProtocolAgentStatusPort,
    ProtocolDefinitionPublicationPort, ProtocolDefinitionRepositoryPort, ProtocolEventCursorPort,
    ProtocolEventStorePort, ProtocolEventSubscriberPort, ProtocolEventTransportPort,
    ProtocolEvidenceSourcePort, ProtocolInstanceIndexPort, ProtocolSnapshotStorePort,
    ProtocolStepHandlerPort, StatisticsPort,
};
use made_core::value_objects::{
    AuthorizationAction, AuthorizationPolicyId, AuthorizationRequestId, ProtocolEventConsumer,
    ProtocolId,
};
use made_core::value_objects::{MaxParallel, ProtocolEventPageLimit};
use std::fmt;
use std::sync::Arc;

mod agent_status;
mod agentic_systems;
mod artifacts;
mod authorization;
mod authorization_guards;
mod budgets;
mod council_journal;
mod councils;
mod definitions;
mod execution;
mod execution_receipts;
mod history;
mod host_handoff;
mod integrator_loop;
mod intervention_delivery;
mod participation;
mod protocol_authority;
mod protocol_operation_authority;
mod protocol_projection_data;
mod succession;

pub use protocol_authority::EmbeddedProtocolAuthority;
pub use protocol_operation_authority::EmbeddedProtocolOperationAuthority;
pub use protocol_projection_data::EmbeddedProtocolProjectionData;

/// In-process facade over the MADE protocol use cases.
#[derive(Clone)]
pub struct EmbeddedMade {
    definitions: Arc<dyn ProtocolDefinitionRepositoryPort>,
    publications: Arc<dyn ProtocolDefinitionPublicationPort>,
    /// The streams themselves, for the one read that wants records
    /// rather than the session they fold to.
    events: Arc<dyn ProtocolEventStorePort>,
    protocol_index: Arc<dyn ProtocolInstanceIndexPort>,
    protocol_search_cursors: Option<ProtocolSearchCursorCodec>,
    protocol_search_authorization: Option<Arc<TrustedHostAuthorizationGate>>,
    progress_stream: Arc<StreamProtocolUseCase>,
    cursors: Arc<dyn ProtocolEventCursorPort>,
    /// A session as the fold of its stream: every verb that reads or
    /// advances one goes through here.
    pub(crate) stream: Arc<SessionStream>,
    step_handler: Arc<dyn ProtocolStepHandlerPort>,
    evidence_source: Arc<dyn ProtocolEvidenceSourcePort>,
    pub(crate) clock: Arc<dyn ClockPort>,
    pub(crate) max_parallel_ceiling: MaxParallel,
    metrics_recorder: Arc<dyn MetricsRecorderPort>,
    metrics_snapshot: Arc<dyn MetricsSnapshotPort>,
    /// Replaceable operational counters; editions without councils honestly stay at zero.
    statistics: Arc<dyn StatisticsPort>,
    councils: Arc<EmbeddedCouncilServices>,
    /// Reads the memory projection written by the stream subscriber (ADR-012).
    memory_reader: Arc<dyn MemoryReaderPort>,
    /// Durable publication is woken after each append and once explicitly at
    /// host startup, so records left pending by a stopped process do not need
    /// another append to move again.
    event_publisher: Option<Arc<PublishProtocolEventsUseCase>>,
    event_publisher_consumer: ProtocolEventConsumer,
    artifacts: Option<Arc<ArtifactService>>,
    execution_receipts: Arc<dyn ExecutionReceiptStorePort>,
    budgets: BudgetLedgerService,
    authorization: Option<EmbeddedAuthorizationServices>,
    agent_status: Arc<made_app::usecases::ProtocolAgentStatusService>,
    host_delivery: HostDeliveryPorts,
    agentic_system: AgenticSystemPorts,
    /// The loop a bound host drives the engine through, and the
    /// projection it reads behind.
    integrator_loop: IntegratorLoopPorts,
}

impl EmbeddedMade {
    /// Open the durable embedded distribution through its public boundary.
    ///
    /// The provider owns the concrete store and its internal ports. A host
    /// receives only the embedded facade, so a storage refactor cannot leak
    /// provider implementation types into the consumer's dependency graph.
    ///
    /// Open the canonical durable SQLite store.
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, ApiError> {
        let path = path.as_ref();
        let store = SqliteProtocolStore::open(path).map_err(|error| ApiError::Unavailable {
            reason: format!("the durable SQLite protocol store did not open: {error}"),
        })?;
        let budgets =
            SqliteBudgetLedgerStore::open(path).map_err(|error| ApiError::Unavailable {
                reason: format!("the durable SQLite budget ledger did not open: {error}"),
            })?;
        Self::over(store, open_artifact_store(path)?, budgets)
    }

    /// Open durable SQLite and publish its global feed after each append.
    pub fn open_with_event_transport(
        path: impl AsRef<std::path::Path>,
        transport: Arc<dyn ProtocolEventTransportPort>,
    ) -> Result<Self, ApiError> {
        let path = path.as_ref();
        let store = SqliteProtocolStore::open(path).map_err(|error| ApiError::Unavailable {
            reason: format!("the durable SQLite protocol store did not open: {error}"),
        })?;
        let store = Arc::new(store);
        Ok(Self::provider_builder(&store)?
            .with_host_delivery_ledger(Arc::new(store.host_delivery_ledger()))
            .with_integrator_bindings(Arc::new(store.integrator_bindings()))
            .with_agentic_system_stores(
                Arc::new(store.agentic_system_repository()),
                Arc::new(store.agentic_system_publications()),
                Arc::new(store.agentic_system_executions()),
            )
            .with_protocol_store_and_memory(store.clone())
            .with_event_cursor(store.clone())
            .with_definition_publications(store)
            .with_artifact_store(Arc::new(open_artifact_store(path)?))
            .with_budget_ledger_store(Arc::new(open_budget_store(path)?))
            .with_event_transport(transport)
            .build())
    }

    /// Open durable SQLite with one operational metrics adapter shared by
    /// event instrumentation and registry reads.
    pub fn open_with_metrics<M>(
        path: impl AsRef<std::path::Path>,
        metrics: Arc<M>,
    ) -> Result<Self, ApiError>
    where
        M: MetricsRecorderPort + MetricsSnapshotPort + 'static,
    {
        Self::open_observed(path.as_ref(), metrics, None, None)
    }

    /// Open durable SQLite with one operational metrics adapter shared by
    /// event instrumentation, registry reads, and an event transport.
    pub fn open_with_observability<M>(
        path: impl AsRef<std::path::Path>,
        metrics: Arc<M>,
        transport: Arc<dyn ProtocolEventTransportPort>,
    ) -> Result<Self, ApiError>
    where
        M: MetricsRecorderPort + MetricsSnapshotPort + 'static,
    {
        Self::open_observed(path.as_ref(), metrics, Some(transport), None)
    }

    /// Open durable SQLite with the activation adapter its operator chose.
    ///
    /// The choice belongs to whoever starts the process, which is why it
    /// arrives as a port rather than being read here: an engine that
    /// reached for the environment on its own would answer discovery
    /// with a decision no caller could see or override in a test.
    pub fn open_with_host_activation<M>(
        path: impl AsRef<std::path::Path>,
        metrics: Arc<M>,
        transport: Option<Arc<dyn ProtocolEventTransportPort>>,
        activation: Arc<dyn HostActivationPort>,
    ) -> Result<Self, ApiError>
    where
        M: MetricsRecorderPort + MetricsSnapshotPort + 'static,
    {
        Self::open_observed(path.as_ref(), metrics, transport, Some(activation))
    }

    fn open_observed<M>(
        path: &std::path::Path,
        metrics: Arc<M>,
        transport: Option<Arc<dyn ProtocolEventTransportPort>>,
        activation: Option<Arc<dyn HostActivationPort>>,
    ) -> Result<Self, ApiError>
    where
        M: MetricsRecorderPort + MetricsSnapshotPort + 'static,
    {
        let store = SqliteProtocolStore::open(path).map_err(|error| ApiError::Unavailable {
            reason: format!("the durable SQLite protocol store did not open: {error}"),
        })?;
        let store = Arc::new(store);
        let mut builder = Self::provider_builder(&store)?
            .with_host_delivery_ledger(Arc::new(store.host_delivery_ledger()))
            .with_integrator_bindings(Arc::new(store.integrator_bindings()))
            .with_agentic_system_stores(
                Arc::new(store.agentic_system_repository()),
                Arc::new(store.agentic_system_publications()),
                Arc::new(store.agentic_system_executions()),
            )
            .with_protocol_store_and_memory(store.clone())
            .with_event_cursor(store.clone())
            .with_definition_publications(store)
            .with_artifact_store(Arc::new(open_artifact_store(path)?))
            .with_budget_ledger_store(Arc::new(open_budget_store(path)?))
            .with_observability(metrics);
        if let Some(transport) = transport {
            builder = builder.with_event_transport(transport);
        }
        if let Some(activation) = activation {
            builder = builder.with_host_activation(activation);
        }
        Ok(builder.build())
    }

    fn over(
        store: SqliteProtocolStore,
        artifacts: LocalArtifactStore,
        budgets: SqliteBudgetLedgerStore,
    ) -> Result<Self, ApiError> {
        let store = Arc::new(store);
        Ok(Self::provider_builder(&store)?
            .with_host_delivery_ledger(Arc::new(store.host_delivery_ledger()))
            .with_integrator_bindings(Arc::new(store.integrator_bindings()))
            .with_agentic_system_stores(
                Arc::new(store.agentic_system_repository()),
                Arc::new(store.agentic_system_publications()),
                Arc::new(store.agentic_system_executions()),
            )
            .with_protocol_store_and_memory(store.clone())
            .with_event_cursor(store.clone())
            .with_definition_publications(store)
            .with_artifact_store(Arc::new(artifacts))
            .with_budget_ledger_store(Arc::new(budgets))
            .build())
    }

    fn provider_builder(store: &SqliteProtocolStore) -> Result<EmbeddedMadeBuilder, ApiError> {
        let factory = DispatchingAgentFactory::from_env().map_err(|error| ApiError::Refused {
            reason: format!("the embedded agent provider configuration is invalid: {error}"),
        })?;
        let factory = Arc::new(factory);
        let councils = SqliteCouncilStore::over(store);
        let journal = Arc::new(SqliteCouncilJournal::new(councils.clone()));
        let builder = Self::builder()
            .with_council_journal(journal)
            .with_agent_factory(factory.clone())
            .with_agent_registry(Arc::new(SqliteAgentRegistry::new(
                councils.clone(),
                factory,
            )))
            .with_council_registry(Arc::new(SqliteCouncilRegistry::new(councils.clone())))
            .with_contract_registry(Arc::new(SqliteContractRegistry::new(councils.clone())))
            .with_deliberation_repository(Arc::new(SqliteDeliberationRepository::new(
                councils.clone(),
            )))
            .with_statistics(Arc::new(SqliteCouncilStatistics::new(councils)));
        Ok(match protocol_search_cursors_from_env()? {
            Some(cursors) => builder.with_protocol_search_cursors(cursors),
            None => builder,
        })
    }

    /// The projection resolves a definition per record, and the
    /// engine's own stream does not exist until the fanout that
    /// projection is installed in does. So it gets a lookup with a
    /// read-only stream over the same two stores, built here.
    fn definition_lookup(
        definitions: &Arc<dyn ProtocolDefinitionRepositoryPort>,
        publications: &Arc<dyn ProtocolDefinitionPublicationPort>,
        events: &Arc<dyn ProtocolEventStorePort>,
        snapshots: &Arc<dyn ProtocolSnapshotStorePort>,
    ) -> Arc<dyn made_app::services::attention::ProtocolDefinitionLookup> {
        Arc::new(SessionDefinitionLookup::over(
            events.clone(),
            snapshots.clone(),
            definitions.clone(),
            publications.clone(),
        ))
    }

    pub(crate) fn new(
        definitions: Arc<dyn ProtocolDefinitionRepositoryPort>,
        publications: Arc<dyn ProtocolDefinitionPublicationPort>,
        events: Arc<dyn ProtocolEventStorePort>,
        protocol_index: Arc<dyn ProtocolInstanceIndexPort>,
        cursors: Arc<dyn ProtocolEventCursorPort>,
        snapshots: Arc<dyn ProtocolSnapshotStorePort>,
        step_handler: Arc<dyn ProtocolStepHandlerPort>,
        evidence_source: Arc<dyn ProtocolEvidenceSourcePort>,
        clock: Arc<dyn ClockPort>,
        max_parallel_ceiling: MaxParallel,
        metrics_recorder: Arc<dyn MetricsRecorderPort>,
        metrics_snapshot: Arc<dyn MetricsSnapshotPort>,
        statistics: Arc<dyn StatisticsPort>,
        councils: Arc<EmbeddedCouncilServices>,
        memory: Arc<dyn MemoryWriterPort>,
        memory_reader: Arc<dyn MemoryReaderPort>,
        subscriber: Option<Arc<dyn ProtocolEventSubscriberPort>>,
        event_transport: Option<Arc<dyn ProtocolEventTransportPort>>,
        progress_settings: ProtocolProgressSettings,
        artifacts: Option<Arc<ArtifactService>>,
        execution_receipts: Arc<dyn ExecutionReceiptStorePort>,
        budgets: BudgetLedgerService,
        protocol_search_cursors: Option<ProtocolSearchCursorCodec>,
        protocol_search_authorization: Option<Arc<TrustedHostAuthorizationGate>>,
        agent_status_port: Arc<dyn ProtocolAgentStatusPort>,
        host_delivery: HostDeliveryPorts,
        agentic_system: AgenticSystemPorts,
    ) -> Self {
        let event_publisher_consumer = ProtocolEventConsumer::new("embedded-file-sink")
            .expect("the embedded sink consumer name is valid");
        let event_publisher = event_transport.map(|transport| {
            Arc::new(PublishProtocolEventsUseCase::new(
                events.clone(),
                cursors.clone(),
                transport,
                clock.clone(),
            ))
        });
        let publisher_subscriber = event_publisher.as_ref().map(|use_case| {
            Arc::new(ProtocolEventPublisherSubscriber::new(
                use_case.clone(),
                event_publisher_consumer.clone(),
            )) as Arc<dyn ProtocolEventSubscriberPort>
        });
        let attention = attention_recovery(
            events.clone(),
            cursors.clone(),
            Self::definition_lookup(&definitions, &publications, &events, &snapshots),
            &host_delivery,
            &agentic_system,
            clock.clone(),
        );
        let progress_notifier = Arc::new(ProtocolProgressNotifier::new());
        let progress_stream = Arc::new(
            StreamProtocolUseCase::with_settings(
                events.clone(),
                progress_notifier.clone(),
                progress_settings,
            )
            .with_agent_activity(agent_status_port.clone()),
        );
        let subscribers = engine_projections::fanout(
            engine_projections::EngineProjections {
                memory,
                events: events.clone(),
                progress: progress_notifier.clone(),
                metrics: metrics_recorder.clone(),
                deliveries: host_delivery.ledger().clone(),
                agent_status: agent_status_port.clone(),
                attention: attention.clone(),
            },
            publisher_subscriber.into_iter().chain(subscriber).collect(),
        );
        let stream = Arc::new(SessionStream::new(events.clone(), snapshots, subscribers));
        let agent_status = Arc::new(
            made_app::usecases::ProtocolAgentStatusService::new(
                agent_status_port,
                clock.clone(),
                time::Duration::seconds(60),
            )
            .with_journal_claims(stream.clone()),
        );
        let integrator_loop = IntegratorLoopPorts::wire(
            attention,
            &host_delivery,
            events.clone(),
            stream.clone(),
            Arc::new(ResolveProtocolDefinitionUseCase::new(
                definitions.clone(),
                publications.clone(),
            )),
            progress_notifier,
            clock.clone(),
        );
        Self {
            definitions,
            publications,
            protocol_index,
            protocol_search_cursors,
            protocol_search_authorization,
            progress_stream,
            stream,
            events,
            cursors,
            step_handler,
            evidence_source,
            clock,
            max_parallel_ceiling,
            metrics_recorder,
            metrics_snapshot,
            statistics,
            councils,
            memory_reader,
            event_publisher,
            event_publisher_consumer,
            artifacts,
            execution_receipts,
            budgets,
            authorization: None,
            agent_status,
            host_delivery,
            agentic_system,
            integrator_loop,
        }
    }

    /// The integrator loop, as this engine composed it.
    #[must_use]
    pub const fn integrator_loop(&self) -> &IntegratorLoopPorts {
        &self.integrator_loop
    }

    /// Work handed out to hosts, as this engine composed it.
    #[must_use]
    pub fn host_delivery_ledger(&self) -> &Arc<dyn HostDeliveryLedgerPort> {
        self.host_delivery.ledger()
    }

    /// Who is driving each protocol, as this engine composed it.
    #[must_use]
    pub fn integrator_bindings(&self) -> &Arc<dyn IntegratorBindingPort> {
        self.host_delivery.bindings()
    }

    /// How a host that does not ask gets woken, if this engine can.
    #[must_use]
    pub fn host_activation(&self) -> &Arc<dyn HostActivationPort> {
        self.host_delivery.activation()
    }

    /// Attach the policy services used by protected direct facade calls and
    /// embedded MCP administration. The caller must pass the same durable
    /// store used by the authorization gate.
    #[must_use]
    pub fn with_authorization_policy(
        mut self,
        policy_id: AuthorizationPolicyId,
        store: Arc<dyn AuthorizationPolicyStorePort>,
    ) -> Self {
        let (memory_reader, authorization) = crate::embedded_authorization_wiring::wire(
            policy_id,
            store,
            self.clock.clone(),
            self.memory_reader.clone(),
            &self.stream,
        );
        self.memory_reader = memory_reader;
        self.authorization = Some(authorization);
        self
    }

    /// Resume durable event publication left pending by an earlier process.
    ///
    /// Constructors stay synchronous and side-effect free beyond opening
    /// adapters. A host calls this once from its async startup path before it
    /// accepts work. The cursor makes repeating recovery safe: acknowledged
    /// records are skipped. A transient delivery failure is retried in this
    /// awaited startup call with bounded backoff; an exhausted delivery is
    /// quarantined by the publisher use case before recovery advances.
    pub async fn recover_event_publication(&self) -> Result<(), DomainError> {
        let Some(publisher) = &self.event_publisher else {
            return Ok(());
        };
        loop {
            let round = publisher
                .execute_automatically(
                    &self.event_publisher_consumer,
                    ProtocolEventPageLimit::DEFAULT,
                )
                .await?;
            if round.busy || round.confirmed() < ProtocolEventPageLimit::DEFAULT.value() {
                return Ok(());
            }
        }
    }

    #[must_use]
    pub fn builder() -> EmbeddedMadeBuilder {
        EmbeddedMadeBuilder::new()
    }

    #[must_use]
    pub const fn version(&self) -> &'static str {
        VERSION
    }

    pub async fn instance(&self, id: &ProtocolId) -> Result<ProtocolInstance, DomainError> {
        self.require_authorized_protocol_action(AuthorizationAction::GetProtocolInstance, id)?;
        GetProtocolInstanceUseCase::new(self.stream.clone())
            .execute(id)
            .await
    }

    pub async fn instances(&self) -> Result<Vec<ProtocolInstance>, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::ListProtocolInstances)?;
        ListProtocolInstancesUseCase::new(self.stream.clone())
            .execute()
            .await
    }

    pub async fn search_instances(
        &self,
        request_id: AuthorizationRequestId,
        input: &SearchProtocolInstancesInput,
    ) -> Result<ProtocolInstancePage, DomainError> {
        crate::embedded_protocol_search::execute(
            self.protocol_search_authorization.as_ref(),
            self.protocol_search_cursors.clone(),
            self.protocol_index.clone(),
            self.stream.clone(),
            request_id,
            input,
        )
        .await
    }

    /// How this engine is doing: the version it was built from, how
    /// long it has been up, its condition, what is recording, and the
    /// counters when they are asked for.
    ///
    /// The same use case the deployable edition's `GetStatus` runs, so
    /// a host that moves between editions reads one answer rather than
    /// two (ADR-014).
    pub async fn status(&self, include_statistics: bool) -> Result<ServiceStatus, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::GetStatus)?;
        GetServiceStatusUseCase::new(
            self.statistics.clone(),
            self.metrics_recorder.clone(),
            VERSION,
            self.clock.clone(),
        )
        .execute(include_statistics)
        .await
    }

    /// The operational counters on their own.
    pub async fn metrics(&self) -> Result<ServiceMetrics, DomainError> {
        self.require_authorized_global_action(AuthorizationAction::GetMetrics)?;
        GetServiceMetricsUseCase::new(self.statistics.clone(), self.metrics_snapshot.clone())
            .execute()
            .await
    }
}

impl Default for EmbeddedMade {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl fmt::Debug for EmbeddedMade {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EmbeddedMade")
            .field("version", &VERSION)
            .finish_non_exhaustive()
    }
}
