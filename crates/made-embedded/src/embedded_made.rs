use crate::{EmbeddedMadeBuilder, VERSION};
use made_adapters::ceremony::{
    CeremonyMetricsSubscriber, CeremonyStructuredLogSubscriber, CeremonyTracingSubscriber,
};
use made_adapters::sqlite::SqliteCeremonyStore;
use made_api::ApiError;
use made_app::services::{
    CeremonyEventFanout, CeremonyEventPublisherSubscriber, SessionMemoryRecorder, SessionStream,
};
use made_app::usecases::{
    GetCeremonyInstanceUseCase, GetServiceMetricsUseCase, GetServiceStatusUseCase,
    ListCeremonyInstancesUseCase, PublishCeremonyEventsUseCase, ServiceStatus,
};
use made_core::entities::{CeremonyInstance, Statistics};
use made_core::error::DomainError;
use made_core::ports::{
    CeremonyDefinitionPublicationPort, CeremonyDefinitionRepositoryPort, CeremonyEventCursorPort,
    CeremonyEventStorePort, CeremonyEventSubscriberPort, CeremonyEventTransportPort,
    CeremonyEvidenceSourcePort, CeremonySnapshotStorePort, CeremonyStepHandlerPort, ClockPort,
    MemoryReaderPort, MemoryWriterPort, MetricsRecorderPort, StatisticsPort,
};
use made_core::value_objects::{CeremonyEventConsumer, CeremonyId};
use std::fmt;
use std::sync::Arc;
use std::time::Instant;

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
    cursors: Arc<dyn CeremonyEventCursorPort>,
    /// A session as the fold of its stream: every verb that reads or
    /// advances one goes through here.
    stream: Arc<SessionStream>,
    step_handler: Arc<dyn CeremonyStepHandlerPort>,
    evidence_source: Arc<dyn CeremonyEvidenceSourcePort>,
    clock: Arc<dyn ClockPort>,
    metrics_recorder: Arc<dyn MetricsRecorderPort>,
    /// The operational counters this engine keeps.
    ///
    /// Wired like every other port so a host can replace it; the
    /// default keeps them in memory, and in an edition that runs no
    /// council they stay at zero — which is the honest answer, not a
    /// missing one.
    statistics: Arc<dyn StatisticsPort>,
    /// When this engine was built. Monotonic, so uptime does not
    /// move when the host's wall clock does.
    started_at: Instant,
    /// The same adapter the recorder writes through, read back.
    ///
    /// The writer side is a subscriber of the stream (ADR-012), so the
    /// recorder is not a field here; this is the read the start use
    /// cases make before a session opens.
    memory_reader: Arc<dyn MemoryReaderPort>,
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
        Ok(Self::over(store))
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
        Ok(Self::builder()
            .with_ceremony_store_and_memory(store.clone())
            .with_event_cursor(store.clone())
            .with_definition_publications(store)
            .with_event_transport(transport)
            .build())
    }

    fn over(store: SqliteCeremonyStore) -> Self {
        let store = Arc::new(store);
        Self::builder()
            .with_ceremony_store_and_memory(store.clone())
            .with_event_cursor(store.clone())
            .with_definition_publications(store)
            .build()
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
        metrics_recorder: Arc<dyn MetricsRecorderPort>,
        statistics: Arc<dyn StatisticsPort>,
        memory: Arc<dyn MemoryWriterPort>,
        memory_reader: Arc<dyn MemoryReaderPort>,
        subscriber: Option<Arc<dyn CeremonyEventSubscriberPort>>,
        event_transport: Option<Arc<dyn CeremonyEventTransportPort>>,
    ) -> Self {
        // What a session leaves behind is a projection of its stream,
        // so it is a subscriber rather than something a use case
        // holds. A host that configures no memory gets one that
        // forgets and says so; handing in a durable writer is the
        // whole of turning it on. The host's own subscriber comes
        // after the engine's.
        let session_memory = Arc::new(SessionMemoryRecorder::new(memory, events.clone()));
        let publisher = event_transport.map(|transport| {
            let use_case = Arc::new(PublishCeremonyEventsUseCase::new(
                events.clone(),
                cursors.clone(),
                transport,
                clock.clone(),
            ));
            Arc::new(CeremonyEventPublisherSubscriber::new(
                use_case,
                CeremonyEventConsumer::new("embedded-file-sink")
                    .expect("the embedded sink consumer name is valid"),
            )) as Arc<dyn CeremonyEventSubscriberPort>
        });
        let mut subscribers: Vec<Arc<dyn CeremonyEventSubscriberPort>> = vec![
            session_memory,
            Arc::new(CeremonyMetricsSubscriber::new(metrics_recorder.clone())),
            Arc::new(CeremonyTracingSubscriber::new()),
            Arc::new(CeremonyStructuredLogSubscriber::new()),
        ];
        subscribers.extend(publisher);
        subscribers.extend(subscriber);
        let subscribers = Arc::new(CeremonyEventFanout::new(subscribers));
        Self {
            definitions,
            publications,
            stream: Arc::new(SessionStream::new(events.clone(), snapshots, subscribers)),
            events,
            cursors,
            step_handler,
            evidence_source,
            clock,
            metrics_recorder,
            statistics,
            started_at: Instant::now(),
            memory_reader,
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
            self.started_at,
        )
        .execute(include_statistics)
        .await
    }

    /// The operational counters on their own.
    pub async fn metrics(&self) -> Result<Statistics, DomainError> {
        GetServiceMetricsUseCase::new(self.statistics.clone())
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
