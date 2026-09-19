//! What a test wants different about the in-process server.
//!
//! `GrpcFixture` mirrors `compose::compose`, and the adapters that
//! composition picks are the ones a deployment gets: a step handler
//! that deliberates through the council, an evidence source that has
//! nothing to read, and the system clock. The parity session needs the
//! server and the in-process edition to run the **same** three, and it
//! cannot get there by changing what production picks — that would
//! prove the test's composition rather than the shipped one.
//!
//! So the choice becomes an argument. Every field left unset keeps
//! exactly what the fixture composed before, so the tests that ask for
//! nothing are unaffected.

use std::sync::Arc;

use made_adapters::memory::{ForgetfulMemory, InMemoryCeremonyEventStore};
use made_adapters::noop::NoopCeremonyEvidenceSource;
use made_core::ports::{
    ArtifactStorePort, CeremonyEventStorePort, CeremonyEvidenceSourcePort,
    CeremonySnapshotStorePort, CeremonyStepHandlerPort, ClockPort, MemoryReaderPort,
    MemoryWriterPort,
};

/// The adapters a fixture will use where a test has an opinion.
pub struct GrpcFixtureWiring {
    ceremony_store: Arc<dyn CeremonyEventStorePort>,
    ceremony_snapshots: Arc<dyn CeremonySnapshotStorePort>,
    step_handler: Option<Arc<dyn CeremonyStepHandlerPort>>,
    evidence_source: Option<Arc<dyn CeremonyEvidenceSourcePort>>,
    clock: Option<Arc<dyn ClockPort>>,
    memory: Option<(Arc<dyn MemoryWriterPort>, Arc<dyn MemoryReaderPort>)>,
    artifact_store: Option<Arc<dyn ArtifactStorePort>>,
}

impl Default for GrpcFixtureWiring {
    fn default() -> Self {
        let store = Arc::new(InMemoryCeremonyEventStore::new());
        Self {
            ceremony_store: store.clone(),
            ceremony_snapshots: store,
            step_handler: None,
            evidence_source: None,
            clock: None,
            memory: None,
            artifact_store: None,
        }
    }
}

impl GrpcFixtureWiring {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Serve over a store another server already wrote to — the
    /// restart boundary inside one process.
    #[must_use]
    pub fn with_ceremony_store<S>(mut self, store: Arc<S>) -> Self
    where
        S: CeremonyEventStorePort + CeremonySnapshotStorePort + 'static,
    {
        self.ceremony_snapshots = store.clone();
        self.ceremony_store = store;
        self
    }

    /// Run steps through this handler instead of deliberating.
    #[must_use]
    pub fn with_step_handler(mut self, handler: Arc<dyn CeremonyStepHandlerPort>) -> Self {
        self.step_handler = Some(handler);
        self
    }

    /// Answer evidence requests from this source instead of failing.
    #[must_use]
    pub fn with_evidence_source(mut self, source: Arc<dyn CeremonyEvidenceSourcePort>) -> Self {
        self.evidence_source = Some(source);
        self
    }

    /// Read the time from here instead of from the system.
    #[must_use]
    pub fn with_clock(mut self, clock: Arc<dyn ClockPort>) -> Self {
        self.clock = Some(clock);
        self
    }

    /// Remember what sessions decide in this memory, and read it back
    /// from the same one.
    ///
    /// One adapter for both directions, as the embedded builder takes
    /// it: a server that wrote to one backend and read from another
    /// would make the parity session compare two engines that disagree
    /// about what they remember for a reason that is not parity.
    #[must_use]
    pub fn with_memory<M>(mut self, memory: Arc<M>) -> Self
    where
        M: MemoryWriterPort + MemoryReaderPort + 'static,
    {
        self.memory = Some((memory.clone(), memory));
        self
    }

    #[must_use]
    pub fn with_artifact_store<S>(mut self, store: Arc<S>) -> Self
    where
        S: ArtifactStorePort + 'static,
    {
        self.artifact_store = Some(store);
        self
    }

    #[must_use]
    pub fn ceremony_store(&self) -> Arc<dyn CeremonyEventStorePort> {
        self.ceremony_store.clone()
    }

    #[must_use]
    pub fn ceremony_snapshots(&self) -> Arc<dyn CeremonySnapshotStorePort> {
        self.ceremony_snapshots.clone()
    }

    /// The handler a test asked for, or the one the composition picks.
    /// Taken as a closure because building the production handler needs
    /// the deliberation use case, which the fixture assembles first.
    #[must_use]
    pub fn step_handler(
        &self,
        composed: impl FnOnce() -> Arc<dyn CeremonyStepHandlerPort>,
    ) -> Arc<dyn CeremonyStepHandlerPort> {
        self.step_handler.clone().unwrap_or_else(composed)
    }

    #[must_use]
    pub fn evidence_source(&self) -> Arc<dyn CeremonyEvidenceSourcePort> {
        self.evidence_source
            .clone()
            .unwrap_or_else(|| Arc::new(NoopCeremonyEvidenceSource::new()))
    }

    #[must_use]
    pub fn clock(&self) -> Arc<dyn ClockPort> {
        self.clock
            .clone()
            .unwrap_or_else(|| Arc::new(made_adapters::clock::SystemClock::new()))
    }

    /// The memory a test asked for, or the one the composition picks:
    /// a backend that keeps nothing and says so.
    #[must_use]
    pub fn memory(&self) -> (Arc<dyn MemoryWriterPort>, Arc<dyn MemoryReaderPort>) {
        self.memory.clone().unwrap_or_else(|| {
            let forgetful = Arc::new(ForgetfulMemory::new());
            (forgetful.clone(), forgetful)
        })
    }

    #[must_use]
    pub fn artifact_store(&self) -> Option<Arc<dyn ArtifactStorePort>> {
        self.artifact_store.clone()
    }
}
