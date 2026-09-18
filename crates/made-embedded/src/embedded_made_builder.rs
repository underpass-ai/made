use std::fmt;
use std::future::Future;
use std::sync::Arc;

use made_adapters::clock::SystemClock;
use made_adapters::memory::ForgetfulMemory;
use made_adapters::memory::{
    InMemoryAgentRegistry, InMemoryCeremonyDefinitionPublications,
    InMemoryCeremonyDefinitionRepository, InMemoryCeremonyEventCursor, InMemoryCeremonyEventStore,
    InMemoryContractRegistry, InMemoryCouncilJournal, InMemoryCouncilRegistry,
    InMemoryDeliberationRepository, InMemoryStatistics,
};
use made_adapters::metrics::PrometheusMetricsRecorder;
use made_adapters::noop::{NoopCeremonyEvidenceSource, NoopCeremonyStepHandler};
use made_adapters::scoring::UniformScoring;
use made_adapters::validators::{
    AllowedStringValuesValidator, BoundedEventShapeValidator, ClaimsEvidenceGroundedValidator,
    ClaimsEvidenceSupportedValidator, ContentNonEmptyValidator, JsonObjectOutputValidator,
    JsonSchemaValidator, RequiredFieldsValidator,
};
use made_app::artifacts::ArtifactService;
use made_app::usecases::CeremonyProgressSettings;
use made_core::entities::CeremonyEvidencePack;
use made_core::error::DomainError;
use made_core::ports::{
    AgentFactoryPort, AgentRegistryPort, AgentResolverPort, ArtifactStorePort,
    CeremonyDefinitionPublicationPort, CeremonyDefinitionRepositoryPort, CeremonyEventCursorPort,
    CeremonyEventStorePort, CeremonyEventSubscriberPort, CeremonyEventTransportPort,
    CeremonyEvidenceRequest, CeremonyEvidenceSourcePort, CeremonySnapshotStorePort,
    CeremonyStepHandlerPort, CeremonyStepHandlerRequest, ClockPort, ContractRegistryPort,
    CouncilRegistryPort, DeliberationRepositoryPort, ExecutorPort, MemoryReaderPort,
    MemoryWriterPort, MessagingPort, MetricsRecorderPort, MetricsSnapshotPort, NoopMetricsRecorder,
    NoopMetricsSnapshot, ScoringPort, StatisticsPort, ValidatorPort,
};
use made_core::value_objects::{MaxParallel, StepResult};

use crate::{
    embedded_council_services::EmbeddedCouncilServices,
    unconfigured_executor::UnconfiguredExecutor, CallbackCeremonyEvidenceSource,
    CallbackCeremonyStepHandler, EmbeddedMade,
};

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
    cursors: Option<Arc<dyn CeremonyEventCursorPort>>,
    snapshots: Option<Arc<dyn CeremonySnapshotStorePort>>,
    subscriber: Option<Arc<dyn CeremonyEventSubscriberPort>>,
    event_transport: Option<Arc<dyn CeremonyEventTransportPort>>,
    step_handler: Option<Arc<dyn CeremonyStepHandlerPort>>,
    evidence_source: Option<Arc<dyn CeremonyEvidenceSourcePort>>,
    clock: Option<Arc<dyn ClockPort>>,
    metrics: Option<Arc<dyn MetricsRecorderPort>>,
    metrics_snapshot: Option<Arc<dyn MetricsSnapshotPort>>,
    statistics: Option<Arc<dyn StatisticsPort>>,
    max_parallel_ceiling: Option<MaxParallel>,
    council_registry: Option<Arc<dyn CouncilRegistryPort>>,
    agent_registry: Option<Arc<dyn AgentRegistryPort>>,
    agent_resolver: Option<Arc<dyn AgentResolverPort>>,
    agent_factory: Option<Arc<dyn AgentFactoryPort>>,
    deliberations: Option<Arc<dyn DeliberationRepositoryPort>>,
    contracts: Option<Arc<dyn ContractRegistryPort>>,
    validators: Option<Vec<Arc<dyn ValidatorPort>>>,
    scoring: Option<Arc<dyn ScoringPort>>,
    executor: Option<Arc<dyn ExecutorPort>>,
    messaging: Option<Arc<dyn MessagingPort>>,
    council_journal: Option<Arc<dyn made_core::ports::CouncilJournalPort>>,
    progress_settings: Option<CeremonyProgressSettings>,
    artifact_store: Option<Arc<dyn ArtifactStorePort>>,
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

    /// Durable named progress for global ceremony-event consumers.
    #[must_use]
    pub fn with_event_cursor(mut self, adapter: Arc<dyn CeremonyEventCursorPort>) -> Self {
        self.cursors = Some(adapter);
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

    /// Deliver the durable global event feed after each successful append.
    #[must_use]
    pub fn with_event_transport(mut self, adapter: Arc<dyn CeremonyEventTransportPort>) -> Self {
        self.event_transport = Some(adapter);
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

    #[must_use]
    pub fn with_max_parallel_ceiling(mut self, ceiling: MaxParallel) -> Self {
        self.max_parallel_ceiling = Some(ceiling);
        self
    }

    #[must_use]
    pub fn with_progress_settings(mut self, settings: CeremonyProgressSettings) -> Self {
        self.progress_settings = Some(settings);
        self
    }

    /// Use the host's durable artifact store through the bounded public service.
    #[must_use]
    pub fn with_artifact_store(mut self, adapter: Arc<dyn ArtifactStorePort>) -> Self {
        self.artifact_store = Some(adapter);
        self
    }

    /// Where operational metrics go.
    ///
    /// Left out, the engine wires its own in-process Prometheus registry. A
    /// host using a recorder that is also readable should prefer
    /// [`Self::with_observability`] so `metrics()` reads this same instance;
    /// this setter changes only the write side.
    #[must_use]
    pub fn with_metrics(mut self, adapter: Arc<dyn MetricsRecorderPort>) -> Self {
        self.metrics = Some(adapter);
        self
    }

    /// Read operational metrics from a host-supplied registry.
    ///
    /// Prefer [`Self::with_observability`] when one adapter implements both
    /// sides. This separate setter exists for hosts whose recorder and reader
    /// are intentionally distinct; those hosts own keeping their identities
    /// aligned.
    #[must_use]
    pub fn with_metrics_snapshot(mut self, adapter: Arc<dyn MetricsSnapshotPort>) -> Self {
        self.metrics_snapshot = Some(adapter);
        self
    }

    /// Record and read operational metrics through the same adapter instance.
    #[must_use]
    pub fn with_observability<M>(mut self, adapter: Arc<M>) -> Self
    where
        M: MetricsRecorderPort + MetricsSnapshotPort + 'static,
    {
        self.metrics = Some(adapter.clone());
        self.metrics_snapshot = Some(adapter);
        self
    }

    /// Where the operational counters a status answer reports live.
    #[must_use]
    pub fn with_statistics(mut self, adapter: Arc<dyn StatisticsPort>) -> Self {
        self.statistics = Some(adapter);
        self
    }

    #[must_use]
    pub fn with_council_registry(mut self, adapter: Arc<dyn CouncilRegistryPort>) -> Self {
        self.council_registry = Some(adapter);
        self
    }

    /// Use one live registry for registration and resolution.
    #[must_use]
    pub fn with_agent_registry<A>(mut self, adapter: Arc<A>) -> Self
    where
        A: AgentRegistryPort + AgentResolverPort + 'static,
    {
        self.agent_registry = Some(adapter.clone());
        self.agent_resolver = Some(adapter);
        self
    }

    /// Use explicitly paired write and resolution ports.
    ///
    /// Most in-process hosts should prefer [`Self::with_agent_registry`],
    /// which guarantees both directions share one adapter. This seam serves
    /// hosts whose registry publishes descriptors to a distinct resolver.
    #[must_use]
    pub fn with_agent_registry_ports(
        mut self,
        registry: Arc<dyn AgentRegistryPort>,
        resolver: Arc<dyn AgentResolverPort>,
    ) -> Self {
        self.agent_registry = Some(registry);
        self.agent_resolver = Some(resolver);
        self
    }

    #[must_use]
    pub fn with_agent_factory(mut self, adapter: Arc<dyn AgentFactoryPort>) -> Self {
        self.agent_factory = Some(adapter);
        self
    }

    #[must_use]
    pub fn with_deliberation_repository(
        mut self,
        adapter: Arc<dyn DeliberationRepositoryPort>,
    ) -> Self {
        self.deliberations = Some(adapter);
        self
    }

    #[must_use]
    pub fn with_contract_registry(mut self, adapter: Arc<dyn ContractRegistryPort>) -> Self {
        self.contracts = Some(adapter);
        self
    }

    #[must_use]
    pub fn with_council_validators(mut self, validators: Vec<Arc<dyn ValidatorPort>>) -> Self {
        self.validators = Some(validators);
        self
    }

    #[must_use]
    pub fn with_council_scoring(mut self, adapter: Arc<dyn ScoringPort>) -> Self {
        self.scoring = Some(adapter);
        self
    }

    #[must_use]
    pub fn with_executor(mut self, adapter: Arc<dyn ExecutorPort>) -> Self {
        self.executor = Some(adapter);
        self
    }

    /// Durable council consumption uses an independent journal and cursor namespace.
    #[must_use]
    pub fn with_council_journal(
        mut self,
        journal: Arc<dyn made_core::ports::CouncilJournalPort>,
    ) -> Self {
        self.council_journal = Some(journal);
        self
    }

    #[must_use]
    pub fn with_messaging(mut self, adapter: Arc<dyn MessagingPort>) -> Self {
        self.messaging = Some(adapter);
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
    pub fn build(mut self) -> EmbeddedMade {
        let definitions = self.definitions.take().unwrap_or_else(|| {
            Arc::new(InMemoryCeremonyDefinitionRepository::new())
                as Arc<dyn CeremonyDefinitionRepositoryPort>
        });
        let publications = self.publications.take().unwrap_or_else(|| {
            Arc::new(InMemoryCeremonyDefinitionPublications::new())
                as Arc<dyn CeremonyDefinitionPublicationPort>
        });
        // Zipped rather than defaulted one at a time, for the reason
        // `with_ceremony_store` takes them together: the pair is set by
        // one call or by neither, and a host that configures nothing
        // still gets one storage behind both.
        let (events, snapshots) = self.events.take().zip(self.snapshots.take()).map_or_else(
            || {
                let store = Arc::new(InMemoryCeremonyEventStore::new());
                (
                    store.clone() as Arc<dyn CeremonyEventStorePort>,
                    store as Arc<dyn CeremonySnapshotStorePort>,
                )
            },
            |(events, snapshots)| (events, snapshots),
        );
        let cursors = self.cursors.take().unwrap_or_else(|| {
            Arc::new(InMemoryCeremonyEventCursor::new()) as Arc<dyn CeremonyEventCursorPort>
        });
        let step_handler = self.step_handler.take().unwrap_or_else(|| {
            Arc::new(NoopCeremonyStepHandler::new()) as Arc<dyn CeremonyStepHandlerPort>
        });
        let evidence_source = self.evidence_source.take().unwrap_or_else(|| {
            Arc::new(NoopCeremonyEvidenceSource::new()) as Arc<dyn CeremonyEvidenceSourcePort>
        });
        let clock = self
            .clock
            .take()
            .unwrap_or_else(|| Arc::new(SystemClock::new()) as Arc<dyn ClockPort>);
        // The default is a real registry, not a sink that forgets.
        // It is in-process and explicit — no global recorder, no
        // exporter, no endpoint — so the cost of the default is a few
        // atomics and the benefit is that a host that wires nothing
        // still has numbers to read. Registering into a registry this
        // call just created can only fail on a duplicate family, which
        // cannot happen here; if it ever did, the engine says `noop`
        // when asked what is recording rather than pretending.
        let (metrics, metrics_snapshot) = match (self.metrics.take(), self.metrics_snapshot.take())
        {
            (None, None) => PrometheusMetricsRecorder::new().map_or_else(
                |_| {
                    let noop = Arc::new(NoopMetricsRecorder);
                    (
                        noop as Arc<dyn MetricsRecorderPort>,
                        Arc::new(NoopMetricsSnapshot) as Arc<dyn MetricsSnapshotPort>,
                    )
                },
                |recorder| {
                    let recorder = Arc::new(recorder);
                    (recorder.clone(), recorder)
                },
            ),
            (metrics, snapshot) => (
                metrics.unwrap_or_else(|| Arc::new(NoopMetricsRecorder)),
                snapshot.unwrap_or_else(|| Arc::new(NoopMetricsSnapshot)),
            ),
        };
        let statistics = self
            .statistics
            .take()
            .unwrap_or_else(|| Arc::new(InMemoryStatistics::new()) as Arc<dyn StatisticsPort>);
        let (memory_writer, memory_reader) = self.memory.take().unwrap_or_else(|| {
            let forgetful = Arc::new(ForgetfulMemory::new());
            (forgetful.clone(), forgetful)
        });
        let council_services =
            self.compose_councils(clock.clone(), statistics.clone(), metrics.clone());
        let artifacts = self
            .artifact_store
            .take()
            .map(ArtifactService::new)
            .map(Arc::new);

        EmbeddedMade::new(
            definitions,
            publications,
            events,
            cursors,
            snapshots,
            step_handler,
            evidence_source,
            clock,
            self.max_parallel_ceiling.unwrap_or(MaxParallel::SERVER_MAX),
            metrics,
            metrics_snapshot,
            statistics,
            council_services,
            memory_writer,
            memory_reader,
            self.subscriber.take(),
            self.event_transport.take(),
            self.progress_settings.unwrap_or_default(),
            artifacts,
        )
    }

    fn compose_councils(
        &mut self,
        clock: Arc<dyn ClockPort>,
        statistics: Arc<dyn StatisticsPort>,
        metrics: Arc<dyn MetricsRecorderPort>,
    ) -> Arc<EmbeddedCouncilServices> {
        let councils = self.council_registry.take().unwrap_or_else(|| {
            Arc::new(InMemoryCouncilRegistry::new()) as Arc<dyn CouncilRegistryPort>
        });
        let (agent_registry, agent_resolver) = self
            .agent_registry
            .take()
            .zip(self.agent_resolver.take())
            .unwrap_or_else(|| {
                let registry = Arc::new(InMemoryAgentRegistry::new());
                (
                    registry.clone() as Arc<dyn AgentRegistryPort>,
                    registry as Arc<dyn AgentResolverPort>,
                )
            });
        let deliberations = self.deliberations.take().unwrap_or_else(|| {
            Arc::new(InMemoryDeliberationRepository::new()) as Arc<dyn DeliberationRepositoryPort>
        });
        let contracts = self.contracts.take().unwrap_or_else(|| {
            Arc::new(InMemoryContractRegistry::new()) as Arc<dyn ContractRegistryPort>
        });
        let journal = self
            .council_journal
            .take()
            .unwrap_or_else(|| Arc::new(InMemoryCouncilJournal::new()));
        let messaging = self.messaging.take().unwrap_or_else(|| {
            Arc::new(
                made_adapters::council_journal_messaging::CouncilJournalMessaging::new(
                    journal.clone(),
                ),
            )
        });
        Arc::new(EmbeddedCouncilServices::new(
            clock,
            journal,
            councils,
            agent_registry,
            agent_resolver,
            self.agent_factory.take().unwrap_or_else(|| {
                Arc::new(made_adapters::agents::DispatchingAgentFactory::new())
                    as Arc<dyn AgentFactoryPort>
            }),
            deliberations,
            contracts,
            self.validators
                .take()
                .unwrap_or_else(default_council_validators),
            self.scoring
                .take()
                .unwrap_or_else(|| Arc::new(UniformScoring::new()) as Arc<dyn ScoringPort>),
            self.executor
                .take()
                .unwrap_or_else(|| Arc::new(UnconfiguredExecutor) as Arc<dyn ExecutorPort>),
            messaging,
            statistics,
            metrics,
        ))
    }
}

impl fmt::Debug for EmbeddedMadeBuilder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EmbeddedMadeBuilder")
            .field("has_definition_repository", &self.definitions.is_some())
            .field("has_ceremony_store", &self.events.is_some())
            .field("has_event_cursor", &self.cursors.is_some())
            .field("has_event_subscriber", &self.subscriber.is_some())
            .field("has_event_transport", &self.event_transport.is_some())
            .field("has_step_handler", &self.step_handler.is_some())
            .field("has_evidence_source", &self.evidence_source.is_some())
            .field("has_clock", &self.clock.is_some())
            .field("has_metrics", &self.metrics.is_some())
            .field("has_metrics_snapshot", &self.metrics_snapshot.is_some())
            .field("has_statistics", &self.statistics.is_some())
            .field("has_memory", &self.memory.is_some())
            .field("has_council_registry", &self.council_registry.is_some())
            .field("has_agent_registry", &self.agent_registry.is_some())
            .field("has_agent_factory", &self.agent_factory.is_some())
            .finish()
    }
}

fn default_council_validators() -> Vec<Arc<dyn ValidatorPort>> {
    vec![
        Arc::new(ContentNonEmptyValidator::new()),
        Arc::new(JsonObjectOutputValidator::new()),
        Arc::new(RequiredFieldsValidator::new()),
        Arc::new(AllowedStringValuesValidator::new()),
        Arc::new(JsonSchemaValidator::new()),
        Arc::new(ClaimsEvidenceGroundedValidator::new()),
        Arc::new(ClaimsEvidenceSupportedValidator::new(None)),
        Arc::new(BoundedEventShapeValidator::new()),
    ]
}
