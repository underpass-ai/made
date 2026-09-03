use std::time::Duration;

use super::{AssertionRecord, BusEnvelopeRecord};

/// Aggregate outcome of a single chain run. Carries everything a
/// caller needs to decide pass/fail without parsing strings.
#[derive(Debug, Clone)]
pub struct ChainOutcome {
    pub chain: &'static str,
    pub contract_id: String,
    pub task_id: Option<String>,
    pub winner_proposal_id: Option<String>,
    /// `Some(true)` — the winning proposal satisfied every validator.
    /// `Some(false)` — Warn mode surfaced a top-ranked candidate that
    ///                 did not satisfy the contract (escalation).
    /// `None` — the chain never reached a deliberation result (e.g.
    ///          the gRPC call errored before the council ran).
    pub validation_passed: Option<bool>,
    pub assertions: Vec<AssertionRecord>,
    pub bus_envelopes: Vec<BusEnvelopeRecord>,
    pub total_duration: Duration,
}

impl ChainOutcome {
    #[must_use]
    pub fn passed(&self) -> bool {
        !self.assertions.iter().any(AssertionRecord::is_failed)
            && self.assertions.iter().any(AssertionRecord::is_passed)
    }

    #[must_use]
    pub fn failed_count(&self) -> usize {
        self.assertions.iter().filter(|a| a.is_failed()).count()
    }

    #[must_use]
    pub fn passed_count(&self) -> usize {
        self.assertions.iter().filter(|a| a.is_passed()).count()
    }

    #[must_use]
    pub fn skipped_count(&self) -> usize {
        self.assertions.iter().filter(|a| a.is_skipped()).count()
    }
}
