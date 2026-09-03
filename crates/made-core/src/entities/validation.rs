//! Validation outcome of a proposal — generic replacement for the
//! SWE-specific `CheckSuite` / `PolicyResult` / `LintResult` / `DryRunResult`
//! of the original service.
//!
//! MADE does not know what a validator checks. A validator
//! is any adapter that, given a proposal, returns a [`ValidatorReport`]
//! (pass/fail + opaque details). A [`ValidationOutcome`] aggregates all
//! reports for a single proposal and carries the final [`Score`].

use serde::{Deserialize, Serialize};

use crate::entities::ValidatorReport;
use crate::value_objects::Score;

/// Aggregate validation outcome for one proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationOutcome {
    score: Score,
    reports: Vec<ValidatorReport>,
}

impl ValidationOutcome {
    #[must_use]
    pub fn new(score: Score, reports: Vec<ValidatorReport>) -> Self {
        Self { score, reports }
    }

    #[must_use]
    pub fn score(&self) -> Score {
        self.score
    }
    #[must_use]
    pub fn reports(&self) -> &[ValidatorReport] {
        &self.reports
    }

    /// Convenience: true iff all reports passed. Independent of score.
    #[must_use]
    pub fn all_passed(&self) -> bool {
        self.reports.iter().all(ValidatorReport::passed)
    }

    /// Convenience: number of failing reports.
    #[must_use]
    pub fn failures(&self) -> usize {
        self.reports.iter().filter(|r| !r.passed()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_objects::Attributes;

    fn report(kind: &str, passed: bool) -> ValidatorReport {
        ValidatorReport::new(kind, passed, "summary", Attributes::empty()).unwrap()
    }

    #[test]
    fn outcome_aggregates_reports() {
        let o = ValidationOutcome::new(
            Score::new(0.75).unwrap(),
            vec![report("lint", true), report("policy", false)],
        );
        assert_eq!(o.reports().len(), 2);
        assert!(!o.all_passed());
        assert_eq!(o.failures(), 1);
        assert_eq!(o.score().get(), 0.75);
    }

    #[test]
    fn outcome_without_failures_reports_all_passed() {
        let o = ValidationOutcome::new(Score::MAX, vec![report("a", true), report("b", true)]);
        assert!(o.all_passed());
        assert_eq!(o.failures(), 0);
    }

    #[test]
    fn outcome_with_no_reports_trivially_passes() {
        let o = ValidationOutcome::new(Score::MIN, vec![]);
        assert!(o.all_passed());
        assert_eq!(o.failures(), 0);
    }
}
