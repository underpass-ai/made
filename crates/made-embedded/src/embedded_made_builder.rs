use std::fmt;
use std::future::Future;
use std::sync::Arc;

use made_adapters::clock::SystemClock;
use made_adapters::memory::ForgetfulMemory;
use made_adapters::memory::{
    InMemoryCeremonyDefinitionPublications, InMemoryCeremonyDefinitionRepository,
    InMemoryCeremonyEventStore, InMemoryStatistics,
};
use made_adapters::metrics::PrometheusMetricsRecorder;
use made_adapters::noop::{NoopCeremonyEvidenceSource, NoopCeremonyStepHandler};
use made_core::entities::CeremonyEvidencePack;
use made_core::error::DomainError;
use made_core::ports::{
    CeremonyDefinitionPublicationPort, CeremonyDefinitionRepositoryPort, CeremonyEventStorePort,
    CeremonyEventSubscriberPort, CeremonyEvidenceRequest, CeremonyEvidenceSourcePort,
    CeremonySnapshotStorePort, CeremonyStepHandlerPort, CeremonyStepHandlerRequest, ClockPort,
    MemoryReaderPort, MemoryWriterPort, MetricsRecorderPort, NoopMetricsRecorder, StatisticsPort,
};
use made_core::value_objects::StepResult;

use crate::{CallbackCeremonyEvidenceSource, CallbackCeremonyStepHandler, EmbeddedMade};

/// Builder for an in-process MADE with replaceable adapters.
#[derive(Default)]
pub struct EmbeddedMadeBuilder {
    /// Where a session's memory goes and where it comes from, if
    /// anywhere.
    ///
    /// Absent means a backend that forgets and says so — the honest
    /// shape of "not configured". A host that wants sessions to be
    /// remembered hands one in here and changes nothing else.
    memory: Option<(Arc<dyn MemoryWriterPort>, Arc<dyn MemoryReaderPort>)>,
    definitions: Option<Arc<dyn CeremonyDefinitionRepositoryPort>>,
    publications: Option<Arc<dyn CeremonyDefinitionPublicationPort>>,
    events: Option<Arc<dyn CeremonyEventStorePort>>,
    snapshots: Option<Arc<dyn CeremonySnapshotStorePort>>,
    subscriber: Option<Arc<dyn CeremonyEventSubscriberPort>>,
    step_handler: Option<Arc<dyn CeremonyStepHandlerPort>>,
    evidence_source: Option<Arc<dyn CeremonyEvidenceSourcePort>>,
    clock: Option<Arc<dyn ClockPort>>,
    metrics: Option<Arc<dyn MetricsRecorderPort>>,
    statistics: Option<Arc<dyn StatisticsPort>>,
}

impl EmbeddedMadeBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_definition_repository(
        mut self,
        adapter: Arc<dyn CeremonyDefinitionRepositoryPort>,
    ) -> Self {
        self.definitions = Some(adapter);
        self
    }

    /// The store published definitions live in.
    ///
    /// Separate from the definition repository on purpose: an instance
    /// started from a definition supplied for the run and one bound to
    /// a published version are not the same act.
    #[must_use]
    pub fn with_definition_publications(
        mut self,
        adapter: Arc<dyn CeremonyDefinitionPublicationPort>,
    ) -> Self {
        self.publications = Some(adapter);
        self
    }

    /// The store a ceremony's stream and its snapshots live in.
    ///
    /// One object serves both ports, and the signature is what makes
    /// that true rather than a note asking hosts to be careful. A
    /// snapshot is a cache of a stream's fold; caching one store's
    /// streams beside another store's is not a configuration a host
    /// should be able to express, because a reopen would fold the
    /// stream from one place and read the cache from another.
    #[must_use]
    pub fn with_ceremony_store<S>(mut self, adapter: Arc<S>) -> Self
    where
        S: CeremonyEventStorePort + CeremonySnapshotStorePort + 'static,
    {
        self.events = Some(adapter.clone());
        self.snapshots = Some(adapter);
        self
    }

    /// Keep ceremony state and session memory in one typed adapter.
    ///
    /// This is the durable Rust-host entry point. Its bounds make a split
    /// composition impossible: the stream, its snapshot cache, memory writes
    /// and memory reads all come from the same object. Hosts that deliberately
    /// compose separate adapters can still use [`Self::with_ceremony_store`]
    /// and [`Self::with_memory`].
    #[must_use]
    pub fn with_ceremony_store_and_memory<S>(mut self, adapter: Arc<S>) -> Self
    where
        S: CeremonyEventStorePort
            + CeremonySnapshotStorePort
            + MemoryWriterPort
            + MemoryReaderPort
            + 'static,
    {
        self.events = Some(adapter.clone());
        self.snapshots = Some(adapter.clone());
        self.memory = Some((adapter.clone(), adapter));
        self
    }

    /// Project something of the host's own from every sealed event.
    ///
    /// The engine's own projections are wired whatever the host does;
    /// this one is told after them, about the records of each append
    /// that landed, in order. It cannot fail a session: the signature
    /// returns nothing, and a subscriber that has something to report
    /// logs it.
    #[must_use]
    pub fn with_event_subscriber(mut self, adapter: Arc<dyn CeremonyEventSubscriberPort>) -> Self {
        self.subscriber = Some(adapter);
        self
    }

    #[must_use]
    pub fn with_step_handler(mut self, adapter: Arc<dyn CeremonyStepHandlerPort>) -> Self {
        self.step_handler = Some(adapter);
        self
    }

    #[must_use]
    pub fn with_step_handler_callback<F, Fut>(self, callback: F) -> Self
    where
        F: Fn(CeremonyStepHandlerRequest) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<StepResult, DomainError>> + Send + 'static,
    {
        self.with_step_handler(Arc::new(CallbackCeremonyStepHandler::new(callback)))
    }

    #[must_use]
    pub fn with_evidence_source(mut self, adapter: Arc<dyn CeremonyEvidenceSourcePort>) -> Self {
        self.evidence_source = Some(adapter);
        self
    }

    #[must_use]
    pub fn with_evidence_source_callback<F, Fut>(self, callback: F) -> Self
    where
        F: Fn(CeremonyEvidenceRequest) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<CeremonyEvidencePack, DomainError>> + Send + 'static,
    {
        self.with_evidence_source(Arc::new(CallbackCeremonyEvidenceSource::new(callback)))
    }

    #[must_use]
    pub fn with_clock(mut self, adapter: Arc<dyn ClockPort>) -> Self {
        self.clock = Some(adapter);
        self
    }

    /// Where operational metrics go.
    ///
    /// Left out, the engine wires its own in-process Prometheus
    /// registry: no endpoint, no exporter, nothing to configure and
    /// nothing leaving the process. A host that already has a registry
    /// hands one in here and gets every observation this engine makes
    /// (plan §3.7 G3).
    #[must_use]
    pub fn with_metrics(mut self, adapter: Arc<dyn MetricsRecorderPort>) -> Self {
        self.metrics = Some(adapter);
        self
    }

    /// Where the operational counters a status answer reports live.
    #[must_use]
    pub fn with_statistics(mut self, adapter: Arc<dyn StatisticsPort>) -> Self {
        self.statistics = Some(adapter);
        self
    }

    /// Keep what sessions decide, and why, in this memory — and read
    /// it back when a later session opens in the same scope.
    ///
    /// Left out, a session records nothing and says so. This is the
    /// whole of turning it on.
    ///
    /// One adapter for both directions, and the signature is what makes
    /// that true rather than a note asking hosts to be careful. A host
    /// that could write to one backend and read from another would get
    /// a memory that never recalls what it wrote — the failure that is
    /// indistinguishable from no memory at all, arrived at through a
    /// configuration nobody would defend out loud.
    #[must_use]
    pub fn with_memory<M>(mut self, memory: Arc<M>) -> Self
    where
        M: MemoryWriterPort + MemoryReaderPort + 'static,
    {
        self.memory = Some((memory.clone(), memory));
        self
    }

    /// Build with in-memory, side-effect-free defaults for every adapter not
    /// supplied by the host.
    #[must_use]
    pub fn build(self) -> EmbeddedMade {
        let definitions = self.definitions.unwrap_or_else(|| {
            Arc::new(InMemoryCeremonyDefinitionRepository::new())
                as Arc<dyn CeremonyDefinitionRepositoryPort>
        });
        let publications = self.publications.unwrap_or_else(|| {
            Arc::new(InMemoryCeremonyDefinitionPublications::new())
                as Arc<dyn CeremonyDefinitionPublicationPort>
        });
        // Zipped rather than defaulted one at a time, for the reason
        // `with_ceremony_store` takes them together: the pair is set by
        // one call or by neither, and a host that configures nothing
        // still gets one storage behind both.
        let (events, snapshots) = self.events.zip(self.snapshots).map_or_else(
            || {
                let store = Arc::new(InMemoryCeremonyEventStore::new());
                (
                    store.clone() as Arc<dyn CeremonyEventStorePort>,
                    store as Arc<dyn CeremonySnapshotStorePort>,
                )
            },
            |(events, snapshots)| (events, snapshots),
        );
        let step_handler = self.step_handler.unwrap_or_else(|| {
            Arc::new(NoopCeremonyStepHandler::new()) as Arc<dyn CeremonyStepHandlerPort>
        });
        let evidence_source = self.evidence_source.unwrap_or_else(|| {
            Arc::new(NoopCeremonyEvidenceSource::new()) as Arc<dyn CeremonyEvidenceSourcePort>
        });
        let clock = self
            .clock
            .unwrap_or_else(|| Arc::new(SystemClock::new()) as Arc<dyn ClockPort>);
        // The default is a real registry, not a sink that forgets.
        // It is in-process and explicit — no global recorder, no
        // exporter, no endpoint — so the cost of the default is a few
        // atomics and the benefit is that a host that wires nothing
        // still has numbers to read. Registering into a registry this
        // call just created can only fail on a duplicate family, which
        // cannot happen here; if it ever did, the engine says `noop`
        // when asked what is recording rather than pretending.
        let metrics = self.metrics.unwrap_or_else(|| {
            PrometheusMetricsRecorder::new().map_or_else(
                |_| Arc::new(NoopMetricsRecorder) as Arc<dyn MetricsRecorderPort>,
                |recorder| Arc::new(recorder) as Arc<dyn MetricsRecorderPort>,
            )
        });
        let statistics = self
            .statistics
            .unwrap_or_else(|| Arc::new(InMemoryStatistics::new()) as Arc<dyn StatisticsPort>);
        let (memory_writer, memory_reader) = self.memory.unwrap_or_else(|| {
            let forgetful = Arc::new(ForgetfulMemory::new());
            (forgetful.clone(), forgetful)
        });

        EmbeddedMade::new(
            definitions,
            publications,
            events,
            snapshots,
            step_handler,
            evidence_source,
            clock,
            metrics,
            statistics,
            memory_writer,
            memory_reader,
            self.subscriber,
        )
    }
}

impl fmt::Debug for EmbeddedMadeBuilder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EmbeddedMadeBuilder")
            .field("has_definition_repository", &self.definitions.is_some())
            .field("has_ceremony_store", &self.events.is_some())
            .field("has_event_subscriber", &self.subscriber.is_some())
            .field("has_step_handler", &self.step_handler.is_some())
            .field("has_evidence_source", &self.evidence_source.is_some())
            .field("has_clock", &self.clock.is_some())
            .field("has_metrics", &self.metrics.is_some())
            .field("has_statistics", &self.statistics.is_some())
            .field("has_memory", &self.memory.is_some())
            .finish()
    }
}
