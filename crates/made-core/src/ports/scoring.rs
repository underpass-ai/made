//! [`ScoringPort`] — domain-agnostic aggregation of validator reports
//! into a single [`Score`] for a proposal.
//!
//! Keeping scoring behind a trait lets operators plug their own policy
//! (weighted average, fail-fast on any failed report, learned
//! combinator, …) without touching the core.

use async_trait::async_trait;

use crate::entities::{RankedOutcome, ValidatorReport};
use crate::error::DomainError;
use crate::value_objects::{Discrimination, Score};

#[async_trait]
pub trait ScoringPort: Send + Sync {
    /// Combine a list of validator reports into a single score.
    async fn score(&self, reports: &[ValidatorReport]) -> Result<Score, DomainError>;

    /// Report whether this policy's ranking re-ordered the winner relative
    /// to a structural baseline — the basis for the judge-discrimination
    /// metric. `ranked` is the policy's final ranking (score-descending).
    ///
    /// The default is `None`: a policy with no notion of a baseline (plain
    /// pass-fraction scoring) has nothing to discriminate. A judge-aware
    /// policy overrides this, comparing its winner against the proposal a
    /// judge-free ranking would have picked. `None` means "do not record",
    /// keeping the metric specific to policies that add a ranking signal.
    fn discrimination(&self, _ranked: &[RankedOutcome]) -> Option<Discrimination> {
        None
    }
}
