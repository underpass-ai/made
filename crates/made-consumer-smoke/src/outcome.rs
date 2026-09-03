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

#[cfg(test)]
use std::time::Duration;

mod assertion_record;
mod assertion_status;
mod bus_envelope_record;
mod chain_outcome;

pub use assertion_record::AssertionRecord;
pub use assertion_status::AssertionStatus;
pub use bus_envelope_record::BusEnvelopeRecord;
pub use chain_outcome::ChainOutcome;

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
