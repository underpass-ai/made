//! Typed assertion + outcome model for the smoke chains.
//!
//! The harness never returns a loose JSON blob. Every chain records
//! its per-step assertion status in [`ChainOutcome::assertions`] and
//! every observed bus envelope in [`ChainOutcome::bus_envelopes`];
//! tests assert on the typed structs directly so a future API change
//! becomes a compile error, not a brittle string match.
//!
//! **A `Skipped` status is not silent.** It always carries a `reason`,
//! and [`ChainOutcome::passed`] does not count a chain as passing
//! when every assertion was skipped — at least one assertion must be
//! `Passed` for the chain to count as exercising anything.

use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssertionStatus {
    Passed,
    Skipped { reason: String },
    Failed { detail: String },
}

#[derive(Debug, Clone)]
pub struct AssertionRecord {
    pub name: &'static str,
    pub status: AssertionStatus,
    pub duration: Duration,
}

impl AssertionRecord {
    #[must_use]
    pub fn passed(name: &'static str, duration: Duration) -> Self {
        Self {
            name,
            status: AssertionStatus::Passed,
            duration,
        }
    }

    #[must_use]
    pub fn skipped(name: &'static str, reason: impl Into<String>) -> Self {
        Self {
            name,
            status: AssertionStatus::Skipped {
                reason: reason.into(),
            },
            duration: Duration::ZERO,
        }
    }

    #[must_use]
    pub fn failed(name: &'static str, detail: impl Into<String>, duration: Duration) -> Self {
        Self {
            name,
            status: AssertionStatus::Failed {
                detail: detail.into(),
            },
            duration,
        }
    }

    #[must_use]
    pub fn is_passed(&self) -> bool {
        matches!(self.status, AssertionStatus::Passed)
    }

    #[must_use]
    pub fn is_failed(&self) -> bool {
        matches!(self.status, AssertionStatus::Failed { .. })
    }

    #[must_use]
    pub fn is_skipped(&self) -> bool {
        matches!(self.status, AssertionStatus::Skipped { .. })
    }
}

/// Snapshot of a bus envelope observed on a subscription during a
/// chain run. Captures only the fields the harness asserts on.
#[derive(Debug, Clone)]
pub struct BusEnvelopeRecord {
    pub subject: String,
    pub event_id: String,
    pub correlation_id: Option<String>,
    pub causation_id: Option<String>,
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passed_requires_at_least_one_passed_assertion() {
        let outcome = ChainOutcome {
            chain: "test",
            contract_id: "c".to_owned(),
            task_id: None,
            winner_proposal_id: None,
            validation_passed: None,
            assertions: vec![AssertionRecord::skipped("only_skip", "reason")],
            bus_envelopes: vec![],
            total_duration: Duration::ZERO,
        };
        assert!(
            !outcome.passed(),
            "an all-skipped outcome must not be reported as passing"
        );
    }

    #[test]
    fn passed_with_passing_assertion_and_skip_is_pass() {
        let outcome = ChainOutcome {
            chain: "test",
            contract_id: "c".to_owned(),
            task_id: None,
            winner_proposal_id: None,
            validation_passed: Some(true),
            assertions: vec![
                AssertionRecord::passed("a", Duration::from_millis(1)),
                AssertionRecord::skipped("b", "no nats"),
            ],
            bus_envelopes: vec![],
            total_duration: Duration::from_millis(1),
        };
        assert!(outcome.passed());
        assert_eq!(outcome.passed_count(), 1);
        assert_eq!(outcome.skipped_count(), 1);
    }

    #[test]
    fn one_failure_taints_the_outcome() {
        let outcome = ChainOutcome {
            chain: "test",
            contract_id: "c".to_owned(),
            task_id: None,
            winner_proposal_id: None,
            validation_passed: None,
            assertions: vec![
                AssertionRecord::passed("a", Duration::from_millis(1)),
                AssertionRecord::failed("b", "boom", Duration::from_millis(1)),
            ],
            bus_envelopes: vec![],
            total_duration: Duration::from_millis(2),
        };
        assert!(!outcome.passed());
        assert_eq!(outcome.failed_count(), 1);
    }
}
