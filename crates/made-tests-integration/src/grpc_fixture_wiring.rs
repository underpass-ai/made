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

use made_adapters::memory::InMemoryCeremonyEventStore;
use made_adapters::noop::NoopCeremonyEvidenceSource;
use made_core::ports::{CeremonyEvidenceSourcePort, CeremonyStepHandlerPort, ClockPort};

/// The adapters a fixture will use where a test has an opinion.
pub struct GrpcFixtureWiring {
    ceremony_store: Arc<InMemoryCeremonyEventStore>,
    step_handler: Option<Arc<dyn CeremonyStepHandlerPort>>,
    evidence_source: Option<Arc<dyn CeremonyEvidenceSourcePort>>,
    clock: Option<Arc<dyn ClockPort>>,
}

impl Default for GrpcFixtureWiring {
    fn default() -> Self {
        Self {
            ceremony_store: Arc::new(InMemoryCeremonyEventStore::new()),
            step_handler: None,
            evidence_source: None,
            clock: None,
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
    pub fn with_ceremony_store(mut self, store: Arc<InMemoryCeremonyEventStore>) -> Self {
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

    #[must_use]
    pub fn ceremony_store(&self) -> Arc<InMemoryCeremonyEventStore> {
        self.ceremony_store.clone()
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
}
