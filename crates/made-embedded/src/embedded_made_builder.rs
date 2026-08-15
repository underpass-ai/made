use std::fmt;
use std::future::Future;
use std::sync::Arc;

use made_adapters::clock::SystemClock;
use made_adapters::memory::ForgetfulMemory;
use made_adapters::memory::{
    InMemoryCeremonyDefinitionPublications, InMemoryCeremonyDefinitionRepository,
    InMemoryCeremonyStore, InMemoryCeremonyTranscriptStore,
};
use made_adapters::noop::{NoopCeremonyEvidenceSource, NoopCeremonyStepHandler};
use made_core::entities::CeremonyEvidencePack;
use made_core::error::DomainError;
use made_core::ports::{
    AuditJournalPort, CeremonyDefinitionPublicationPort, CeremonyDefinitionRepositoryPort,
    CeremonyEvidenceRequest, CeremonyEvidenceSourcePort, CeremonyInstanceRepositoryPort,
    CeremonyStepHandlerPort, CeremonyStepHandlerRequest, CeremonyTranscriptStorePort,
    CeremonyUnitOfWorkPort, ClockPort, MemoryWriterPort, MetricsRecorderPort, NoopMetricsRecorder,
};
use made_core::value_objects::StepResult;

use crate::{CallbackCeremonyEvidenceSource, CallbackCeremonyStepHandler, EmbeddedMade};

/// Builder for an in-process MADE with replaceable adapters.
#[derive(Default)]
pub struct EmbeddedMadeBuilder {
    /// Where a session's memory goes, if anywhere.
    ///
    /// Absent means a backend that forgets and says so — the honest
    /// shape of "not configured". A host that wants sessions to be
    /// remembered hands one in here and changes nothing else.
    memory: Option<Arc<dyn MemoryWriterPort>>,
    definitions: Option<Arc<dyn CeremonyDefinitionRepositoryPort>>,
    publications: Option<Arc<dyn CeremonyDefinitionPublicationPort>>,
    instances: Option<Arc<dyn CeremonyInstanceRepositoryPort>>,
    unit_of_work: Option<Arc<dyn CeremonyUnitOfWorkPort>>,
    audit_journal: Option<Arc<dyn AuditJournalPort>>,
    transcript_store: Option<Arc<dyn CeremonyTranscriptStorePort>>,
    step_handler: Option<Arc<dyn CeremonyStepHandlerPort>>,
    evidence_source: Option<Arc<dyn CeremonyEvidenceSourcePort>>,
    clock: Option<Arc<dyn ClockPort>>,
    metrics: Option<Arc<dyn MetricsRecorderPort>>,
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

    /// The store sessions are read from and committed to.
    ///
    /// One object serves both ports, and the signature is what makes
    /// that true rather than a note asking hosts to be careful. Reading
    /// state from one storage while committing it to another is not a
    /// configuration a host should be able to express: the commit would
    /// land, the read would not see it, and every port would look
    /// correctly implemented.
    #[must_use]
    pub fn with_ceremony_store<S>(mut self, adapter: Arc<S>) -> Self
    where
        S: AuditJournalPort + CeremonyInstanceRepositoryPort + CeremonyUnitOfWorkPort + 'static,
    {
        self.instances = Some(adapter.clone());
        self.unit_of_work = Some(adapter.clone());
        self.audit_journal = Some(adapter);
        self
    }

    #[must_use]
    pub fn with_transcript_store(mut self, adapter: Arc<dyn CeremonyTranscriptStorePort>) -> Self {
        self.transcript_store = Some(adapter);
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
    pub fn with_metrics(mut self, adapter: Arc<dyn MetricsRecorderPort>) -> Self {
        self.metrics = Some(adapter);
        self
    }

    /// Keep what sessions decide, and why, in this memory.
    ///
    /// Left out, a session records nothing and says so. This is the
    /// whole of turning it on.
    #[must_use]
    pub fn with_memory(mut self, memory: Arc<dyn MemoryWriterPort>) -> Self {
        self.memory = Some(memory);
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
        let (instances, unit_of_work, audit_journal) = self
            .instances
            .zip(self.unit_of_work)
            .zip(self.audit_journal)
            .map_or_else(
                || {
                    let store = Arc::new(InMemoryCeremonyStore::new());
                    (
                        store.clone() as Arc<dyn CeremonyInstanceRepositoryPort>,
                        store.clone() as Arc<dyn CeremonyUnitOfWorkPort>,
                        store as Arc<dyn AuditJournalPort>,
                    )
                },
                |((instances, unit_of_work), audit_journal)| {
                    (instances, unit_of_work, audit_journal)
                },
            );
        let transcript_store = self.transcript_store.unwrap_or_else(|| {
            Arc::new(InMemoryCeremonyTranscriptStore::new()) as Arc<dyn CeremonyTranscriptStorePort>
        });
        let step_handler = self.step_handler.unwrap_or_else(|| {
            Arc::new(NoopCeremonyStepHandler::new()) as Arc<dyn CeremonyStepHandlerPort>
        });
        let evidence_source = self.evidence_source.unwrap_or_else(|| {
            Arc::new(NoopCeremonyEvidenceSource::new()) as Arc<dyn CeremonyEvidenceSourcePort>
        });
        let clock = self
            .clock
            .unwrap_or_else(|| Arc::new(SystemClock::new()) as Arc<dyn ClockPort>);
        let metrics = self
            .metrics
            .unwrap_or_else(|| Arc::new(NoopMetricsRecorder) as Arc<dyn MetricsRecorderPort>);

        EmbeddedMade::new(
            definitions,
            publications,
            instances,
            unit_of_work,
            audit_journal,
            transcript_store,
            step_handler,
            evidence_source,
            clock,
            metrics,
            self.memory
                .unwrap_or_else(|| Arc::new(ForgetfulMemory::new())),
        )
    }
}

impl fmt::Debug for EmbeddedMadeBuilder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EmbeddedMadeBuilder")
            .field("has_definition_repository", &self.definitions.is_some())
            .field("has_ceremony_store", &self.instances.is_some())
            .field("has_transcript_store", &self.transcript_store.is_some())
            .field("has_step_handler", &self.step_handler.is_some())
            .field("has_evidence_source", &self.evidence_source.is_some())
            .field("has_clock", &self.clock.is_some())
            .field("has_metrics", &self.metrics.is_some())
            .finish()
    }
}
