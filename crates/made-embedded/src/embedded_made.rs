use std::fmt;
use std::sync::Arc;
use std::time::Instant;

use made_adapters::sqlite::SqliteCeremonyStore;
use made_api::ApiError;
use made_app::services::{
    CeremonyEventFanout, CeremonyEventPublisherSubscriber, SessionMemoryRecorder, SessionStream,
};
use made_app::usecases::{
    ApplyCeremonyTransitionInput, ApplyCeremonyTransitionUseCase, ApproveCeremonyGuardInput,
    ApproveCeremonyGuardUseCase, AssertCeremonyReasonInput, AssertCeremonyReasonUseCase,
    BindCeremonyParticipantsInput, BindCeremonyParticipantsUseCase, CeremonyDefinitionSource,
    CeremonyDesignDocument, CloseCeremonyInterventionInput, CloseCeremonyInterventionUseCase,
    CollectCeremonyEvidenceInput, CollectCeremonyEvidenceUseCase, CompleteCeremonyStepInput,
    CompleteCeremonyStepUseCase, DeferCeremonyGuardInput, DeferCeremonyGuardUseCase,
    DesignCeremonyUseCase, DesignedCeremony, DiffCeremonyDefinitionsUseCase,
    GetCeremonyDefinitionUseCase, GetCeremonyInstanceUseCase, GetServiceMetricsUseCase,
    GetServiceStatusUseCase, ListCeremonyDefinitionsUseCase, ListCeremonyInstancesUseCase,
    MountCeremonyDefinitionsOutput, MountCeremonyDefinitionsUseCase,
    PublishCeremonyDefinitionUseCase, PublishCeremonyEventsUseCase,
    RequestCeremonyInterventionInput, RequestCeremonyInterventionUseCase,
    ResolveCeremonyDefinitionUseCase, RespondToCeremonyInterventionInput,
    RespondToCeremonyInterventionUseCase, RunCeremonyInput, RunCeremonyOutput,
    RunCeremonyStepInput, RunCeremonyStepOutput, RunCeremonyStepUseCase, RunCeremonyUseCase,
    ServiceStatus, StartCeremonyInput, StartCeremonyStepInput, StartCeremonyStepUseCase,
    StartCeremonyUseCase, StartPublishedCeremonyUseCase,
};
use made_core::entities::{
    CeremonyDefinition, CeremonyInstance, PublicationOutcome, PublishedCeremonyDefinition,
    Statistics,
};
use made_core::error::DomainError;
use made_core::ports::{
    CeremonyDefinitionPublicationPort, CeremonyDefinitionRepositoryPort, CeremonyEventCursorPort,
    CeremonyEventStorePort, CeremonyEventSubscriberPort, CeremonyEventTransportPort,
    CeremonyEvidenceSourcePort, CeremonySnapshotStorePort, CeremonyStepHandlerPort, ClockPort,
    MemoryReaderPort, MemoryWriterPort, MetricsRecorderPort, StatisticsPort,
};
use made_core::value_objects::{
    CeremonyDefinitionDiff, CeremonyEventConsumer, CeremonyId, CeremonyName, CeremonyVersion,
    StepAttempt,
};

mod history;

use crate::{EmbeddedMadeBuilder, InProcessCeremonyDefinitionSource, VERSION};

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
        let subscribers = Arc::new(CeremonyEventFanout::new(
            core::iter::once(session_memory as Arc<dyn CeremonyEventSubscriberPort>)
                .chain(publisher)
                .chain(subscriber)
                .collect(),
        ));
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

    pub async fn mount_definition(
        &self,
        definition: CeremonyDefinition,
    ) -> Result<MountCeremonyDefinitionsOutput, DomainError> {
        self.mount_definitions([definition]).await
    }

    pub async fn mount_definitions(
        &self,
        definitions: impl IntoIterator<Item = CeremonyDefinition>,
    ) -> Result<MountCeremonyDefinitionsOutput, DomainError> {
        let source = Arc::new(InProcessCeremonyDefinitionSource::new(definitions));
        MountCeremonyDefinitionsUseCase::new(source, self.definitions.clone())
            .execute()
            .await
    }

    pub async fn mount_yaml(
        &self,
        raw: &str,
    ) -> Result<MountCeremonyDefinitionsOutput, DomainError> {
        let source = Arc::new(InProcessCeremonyDefinitionSource::from_yaml(raw)?);
        MountCeremonyDefinitionsUseCase::new(source, self.definitions.clone())
            .execute()
            .await
    }

    pub async fn definition(
        &self,
        name: &CeremonyName,
        version: &CeremonyVersion,
    ) -> Result<CeremonyDefinition, DomainError> {
        GetCeremonyDefinitionUseCase::new(self.definitions.clone())
            .execute(name, version)
            .await
    }

    pub async fn definitions(&self) -> Result<Vec<CeremonyDefinition>, DomainError> {
        ListCeremonyDefinitionsUseCase::new(self.definitions.clone())
            .execute()
            .await
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

    pub async fn run(&self, input: RunCeremonyInput) -> Result<RunCeremonyOutput, DomainError> {
        RunCeremonyUseCase::new(
            self.definitions.clone(),
            self.stream.clone(),
            self.step_handler.clone(),
            self.clock.clone(),
        )
        .with_metrics(self.metrics_recorder.clone())
        .execute(input)
        .await
    }

    /// Fix a definition to an immutable version.
    pub async fn publish_definition(
        &self,
        definition: CeremonyDefinition,
    ) -> Result<PublicationOutcome, DomainError> {
        PublishCeremonyDefinitionUseCase::new(self.publications.clone())
            .execute(definition)
            .await
    }

    /// The published definition under a name and version, if any.
    pub async fn published_definition(
        &self,
        name: &CeremonyName,
        version: &CeremonyVersion,
    ) -> Result<Option<PublishedCeremonyDefinition>, DomainError> {
        self.publications.published(name, version).await
    }

    /// Every published definition.
    pub async fn published_definitions(
        &self,
    ) -> Result<Vec<PublishedCeremonyDefinition>, DomainError> {
        self.publications.catalogue().await
    }

    /// The definition an instance actually runs, binding included.
    ///
    /// Delegates to the shared use case rather than holding the rule,
    /// so the embedded and deployable distributions cannot drift apart
    /// on what a bound instance means.
    pub async fn definition_for(
        &self,
        instance: &CeremonyInstance,
    ) -> Result<CeremonyDefinition, DomainError> {
        self.resolve_definition().execute(instance).await
    }

    /// How every verb that advances a session finds what it is running.
    /// A bound session resolves from the catalogue and is checked
    /// against the digest it recorded; an unbound one has only the
    /// repository. Handing this to the use cases is what lets a
    /// published session be advanced at all.
    fn resolve_definition(&self) -> Arc<ResolveCeremonyDefinitionUseCase> {
        Arc::new(ResolveCeremonyDefinitionUseCase::new(
            self.definitions.clone(),
            self.publications.clone(),
        ))
    }

    /// Seat this session's roles.
    pub async fn bind_participants(
        &self,
        input: BindCeremonyParticipantsInput,
    ) -> Result<CeremonyInstance, DomainError> {
        BindCeremonyParticipantsUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    /// Turn authoring intent into a ceremony document.
    ///
    /// Touches nothing, like validating and explaining a draft: it
    /// reads no store, writes no definition and starts no session.
    /// What it answers with is the document an author can then put
    /// through those three.
    // A method rather than an associated function: a host asks the
    // engine it holds, and designing is one of the things it asks.
    // That it needs nothing from the engine today is a fact about
    // designing, not about where the question belongs.
    #[allow(clippy::unused_self)]
    pub fn design(
        &self,
        document: &CeremonyDesignDocument,
    ) -> Result<DesignedCeremony, DomainError> {
        DesignCeremonyUseCase::new().execute(document)
    }

    /// Compare two definitions, either side published or supplied.
    pub async fn diff_definitions(
        &self,
        before: CeremonyDefinitionSource,
        after: CeremonyDefinitionSource,
    ) -> Result<CeremonyDefinitionDiff, DomainError> {
        DiffCeremonyDefinitionsUseCase::new(self.publications.clone())
            .execute(before, after)
            .await
    }

    /// Start an instance bound to a published definition's digest.
    pub async fn start_published(
        &self,
        input: StartCeremonyInput,
    ) -> Result<CeremonyInstance, DomainError> {
        StartPublishedCeremonyUseCase::new(
            self.publications.clone(),
            self.stream.clone(),
            self.clock.clone(),
            self.memory_reader.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn start(&self, input: StartCeremonyInput) -> Result<CeremonyInstance, DomainError> {
        StartCeremonyUseCase::new(
            self.definitions.clone(),
            self.stream.clone(),
            self.clock.clone(),
            self.memory_reader.clone(),
        )
        .execute(input)
        .await
    }

    /// Say why one thing this session produced led to another.
    ///
    /// In-process only for now, and deliberately: a host embedding the
    /// engine can record its reasoning today without a wire format
    /// being settled for it.
    pub async fn assert_reason(
        &self,
        input: AssertCeremonyReasonInput,
    ) -> Result<CeremonyInstance, DomainError> {
        AssertCeremonyReasonUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn approve_guard(
        &self,
        input: ApproveCeremonyGuardInput,
    ) -> Result<CeremonyInstance, DomainError> {
        ApproveCeremonyGuardUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn defer_guard(
        &self,
        input: DeferCeremonyGuardInput,
    ) -> Result<CeremonyInstance, DomainError> {
        DeferCeremonyGuardUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn request_intervention(
        &self,
        input: RequestCeremonyInterventionInput,
    ) -> Result<CeremonyInstance, DomainError> {
        RequestCeremonyInterventionUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn respond_to_intervention(
        &self,
        input: RespondToCeremonyInterventionInput,
    ) -> Result<CeremonyInstance, DomainError> {
        RespondToCeremonyInterventionUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn collect_evidence(
        &self,
        input: CollectCeremonyEvidenceInput,
    ) -> Result<CeremonyInstance, DomainError> {
        CollectCeremonyEvidenceUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.evidence_source.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn close_intervention(
        &self,
        input: CloseCeremonyInterventionInput,
    ) -> Result<CeremonyInstance, DomainError> {
        CloseCeremonyInterventionUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn start_step(
        &self,
        input: StartCeremonyStepInput,
    ) -> Result<StepAttempt, DomainError> {
        StartCeremonyStepUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn run_step(
        &self,
        input: RunCeremonyStepInput,
    ) -> Result<RunCeremonyStepOutput, DomainError> {
        RunCeremonyStepUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.step_handler.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn complete_step(
        &self,
        input: CompleteCeremonyStepInput,
    ) -> Result<CeremonyInstance, DomainError> {
        CompleteCeremonyStepUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
        .await
    }

    pub async fn apply_transition(
        &self,
        input: ApplyCeremonyTransitionInput,
    ) -> Result<CeremonyInstance, DomainError> {
        ApplyCeremonyTransitionUseCase::new(
            self.resolve_definition(),
            self.stream.clone(),
            self.clock.clone(),
        )
        .execute(input)
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
