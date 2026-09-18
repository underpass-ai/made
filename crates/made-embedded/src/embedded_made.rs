use crate::{embedded_council_services::EmbeddedCouncilServices, EmbeddedMadeBuilder, VERSION};
use made_adapters::agents::DispatchingAgentFactory;
use made_adapters::ceremony::{
    CeremonyFanoutMetricsSubscriber, CeremonyMetricsSubscriber, CeremonyStructuredLogSubscriber,
    CeremonyTracingSubscriber,
};
use made_adapters::progress::CeremonyProgressNotifier;
use made_adapters::sqlite::{
    SqliteAgentRegistry, SqliteCeremonyStore, SqliteContractRegistry, SqliteCouncilJournal,
    SqliteCouncilRegistry, SqliteCouncilStatistics, SqliteCouncilStore,
    SqliteDeliberationRepository,
};
use made_api::ApiError;
use made_app::services::{
    CeremonyEventFanout, CeremonyEventPublisherSubscriber, SessionMemoryRecorder, SessionStream,
};
use made_app::usecases::{
    CeremonyProgressSettings, GetCeremonyInstanceUseCase, GetServiceMetricsUseCase,
    GetServiceStatusUseCase, ListCeremonyInstancesUseCase, PublishCeremonyEventsUseCase,
    ServiceMetrics, ServiceStatus, StreamCeremonyUseCase,
};
use made_core::entities::CeremonyInstance;
use made_core::error::DomainError;
use made_core::ports::{
    CeremonyDefinitionPublicationPort, CeremonyDefinitionRepositoryPort, CeremonyEventCursorPort,
    CeremonyEventStorePort, CeremonyEventSubscriberPort, CeremonyEventTransportPort,
    CeremonyEvidenceSourcePort, CeremonySnapshotStorePort, CeremonyStepHandlerPort, ClockPort,
    MemoryReaderPort, MemoryWriterPort, MetricsRecorderPort, MetricsSnapshotPort, StatisticsPort,
};
use made_core::value_objects::{CeremonyEventConsumer, CeremonyId};
use made_core::value_objects::{CeremonyEventPageLimit, MaxParallel};
use std::fmt;
use std::sync::Arc;

mod councils;
mod definitions;
mod execution;
mod history;
mod participation;

/// In-process facade over the MADE ceremony use cases.
#[derive(Clone)]
pub struct EmbeddedMade {
    definitions: Arc<dyn CeremonyDefinitionRepositoryPort>,
    publications: Arc<dyn CeremonyDefinitionPublicationPort>,
    /// The streams themselves, for the one read that wants records
    /// rather than the session they fold to.
    events: Arc<dyn CeremonyEventStorePort>,
    progress_stream: Arc<StreamCeremonyUseCase>,
    cursors: Arc<dyn CeremonyEventCursorPort>,
    /// A session as the fold of its stream: every verb that reads or
    /// advances one goes through here.
    stream: Arc<SessionStream>,
    step_handler: Arc<dyn CeremonyStepHandlerPort>,
    evidence_source: Arc<dyn CeremonyEvidenceSourcePort>,
    pub(crate) clock: Arc<dyn ClockPort>,
    pub(crate) max_parallel_ceiling: MaxParallel,
    metrics_recorder: Arc<dyn MetricsRecorderPort>,
    metrics_snapshot: Arc<dyn MetricsSnapshotPort>,
    /// The operational counters this engine keeps.
    ///
    /// Wired like every other port so a host can replace it; the
    /// default keeps them in memory, and in an edition that runs no
    /// council they stay at zero — which is the honest answer, not a
    /// missing one.
    statistics: Arc<dyn StatisticsPort>,
    councils: Arc<EmbeddedCouncilServices>,
    /// The same adapter the recorder writes through, read back.
    ///
    /// The writer side is a subscriber of the stream (ADR-012), so the
    /// recorder is not a field here; this is the read the start use
    /// cases make before a session opens.
    memory_reader: Arc<dyn MemoryReaderPort>,
    /// Durable publication is woken after each append and once explicitly at
    /// host startup, so records left pending by a stopped process do not need
    /// another append to move again.
    event_publisher: Option<Arc<PublishCeremonyEventsUseCase>>,
    event_publisher_consumer: CeremonyEventConsumer,
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
        let store = SqliteCeremonyStore::open(path).map_err(|error| ApiError::Unavailable {
            reason: format!("the durable SQLite ceremony store did not open: {error}"),
        })?;
        Self::over(store)
    }

    /// Open durable SQLite and publish its global feed after each append.
    pub fn open_with_event_transport(
        path: impl AsRef<std::path::Path>,
        transport: Arc<dyn CeremonyEventTransportPort>,
    ) -> Result<Self, ApiError> {
        let store = SqliteCeremonyStore::open(path).map_err(|error| ApiError::Unavailable {
            reason: format!("the durable SQLite ceremony store did not open: {error}"),
        })?;
        let store = Arc::new(store);
        Ok(Self::provider_builder(&store)?
            .with_ceremony_store_and_memory(store.clone())
            .with_event_cursor(store.clone())
            .with_definition_publications(store)
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
        let store = SqliteCeremonyStore::open(path).map_err(|error| ApiError::Unavailable {
            reason: format!("the durable SQLite ceremony store did not open: {error}"),
        })?;
        let store = Arc::new(store);
        Ok(Self::provider_builder(&store)?
            .with_ceremony_store_and_memory(store.clone())
            .with_event_cursor(store.clone())
            .with_definition_publications(store)
            .with_observability(metrics)
            .build())
    }

    /// Open durable SQLite with one operational metrics adapter shared by
    /// event instrumentation, registry reads, and an event transport.
    pub fn open_with_observability<M>(
        path: impl AsRef<std::path::Path>,
        metrics: Arc<M>,
        transport: Arc<dyn CeremonyEventTransportPort>,
    ) -> Result<Self, ApiError>
    where
        M: MetricsRecorderPort + MetricsSnapshotPort + 'static,
    {
        let store = SqliteCeremonyStore::open(path).map_err(|error| ApiError::Unavailable {
            reason: format!("the durable SQLite ceremony store did not open: {error}"),
        })?;
        let store = Arc::new(store);
        Ok(Self::provider_builder(&store)?
            .with_ceremony_store_and_memory(store.clone())
            .with_event_cursor(store.clone())
            .with_definition_publications(store)
            .with_observability(metrics)
            .with_event_transport(transport)
            .build())
    }

    fn over(store: SqliteCeremonyStore) -> Result<Self, ApiError> {
        let store = Arc::new(store);
        Ok(Self::provider_builder(&store)?
            .with_ceremony_store_and_memory(store.clone())
            .with_event_cursor(store.clone())
            .with_definition_publications(store)
            .build())
    }

    fn provider_builder(store: &SqliteCeremonyStore) -> Result<EmbeddedMadeBuilder, ApiError> {
        let factory = DispatchingAgentFactory::from_env().map_err(|error| ApiError::Refused {
            reason: format!("the embedded agent provider configuration is invalid: {error}"),
        })?;
        let factory = Arc::new(factory);
        let councils = SqliteCouncilStore::over(store);
        let journal = Arc::new(SqliteCouncilJournal::new(councils.clone()));
        Ok(Self::builder()
            .with_messaging(Arc::new(
                made_adapters::council_journal_messaging::CouncilJournalMessaging::new(journal),
            ))
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
            .with_statistics(Arc::new(SqliteCouncilStatistics::new(councils))))
    }

    pub(crate) fn new(
        definitions: Arc<dyn CeremonyDefinitionRepositoryPort>,
        publications: Arc<dyn CeremonyDefinitionPublicationPort>,
        events: Arc<dyn CeremonyEventStorePort>,
        cursors: Arc<dyn CeremonyEventCursorPort>,
        snapshots: Arc<dyn CeremonySnapshotStorePort>,
        step_handler: Arc<dyn CeremonyStepHandlerPort>,
        evidence_source: Arc<dyn CeremonyEvidenceSourcePort>,
        clock: Arc<dyn ClockPort>,
        max_parallel_ceiling: MaxParallel,
        metrics_recorder: Arc<dyn MetricsRecorderPort>,
        metrics_snapshot: Arc<dyn MetricsSnapshotPort>,
        statistics: Arc<dyn StatisticsPort>,
        councils: Arc<EmbeddedCouncilServices>,
        memory: Arc<dyn MemoryWriterPort>,
        memory_reader: Arc<dyn MemoryReaderPort>,
        subscriber: Option<Arc<dyn CeremonyEventSubscriberPort>>,
        event_transport: Option<Arc<dyn CeremonyEventTransportPort>>,
        progress_settings: CeremonyProgressSettings,
    ) -> Self {
        // What a session leaves behind is a projection of its stream,
        // so it is a subscriber rather than something a use case
        // holds. A host that configures no memory gets one that
        // forgets and says so; handing in a durable writer is the
        // whole of turning it on. The host's own subscriber comes
        // after the engine's.
        let session_memory = Arc::new(SessionMemoryRecorder::new(memory, events.clone()));
        let event_publisher_consumer = CeremonyEventConsumer::new("embedded-file-sink")
            .expect("the embedded sink consumer name is valid");
        let event_publisher = event_transport.map(|transport| {
            Arc::new(PublishCeremonyEventsUseCase::new(
                events.clone(),
                cursors.clone(),
                transport,
                clock.clone(),
            ))
        });
        let publisher_subscriber = event_publisher.as_ref().map(|use_case| {
            Arc::new(CeremonyEventPublisherSubscriber::new(
                use_case.clone(),
                event_publisher_consumer.clone(),
            )) as Arc<dyn CeremonyEventSubscriberPort>
        });
        let progress_notifier = Arc::new(CeremonyProgressNotifier::new());
        let progress_stream = Arc::new(StreamCeremonyUseCase::with_settings(
            events.clone(),
            progress_notifier.clone(),
            progress_settings,
        ));
        let mut subscribers: Vec<Arc<dyn CeremonyEventSubscriberPort>> = vec![
            session_memory,
            progress_notifier,
            Arc::new(CeremonyMetricsSubscriber::new(metrics_recorder.clone())),
            Arc::new(CeremonyFanoutMetricsSubscriber::new(
                events.clone(),
                metrics_recorder.clone(),
            )),
            Arc::new(CeremonyTracingSubscriber::new()),
            Arc::new(CeremonyStructuredLogSubscriber::new()),
        ];
        subscribers.extend(publisher_subscriber);
        subscribers.extend(subscriber);
        let subscribers = Arc::new(CeremonyEventFanout::new(subscribers));
        Self {
            definitions,
            publications,
            progress_stream,
            stream: Arc::new(SessionStream::new(events.clone(), snapshots, subscribers)),
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
        }
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
                    CeremonyEventPageLimit::DEFAULT,
                )
                .await?;
            if round.busy || round.confirmed() < CeremonyEventPageLimit::DEFAULT.value() {
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

    pub async fn instance(&self, id: &CeremonyId) -> Result<CeremonyInstance, DomainError> {
        GetCeremonyInstanceUseCase::new(self.stream.clone())
            .execute(id)
            .await
    }

    pub async fn instances(&self) -> Result<Vec<CeremonyInstance>, DomainError> {
        ListCeremonyInstancesUseCase::new(self.stream.clone())
            .execute()
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
