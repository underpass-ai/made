use serde::{Deserialize, Serialize};

use crate::entities::{Proposal, ValidationOutcome};

/// A proposal paired with its validation outcome and final rank.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankedOutcome {
    proposal: Proposal,
    outcome: ValidationOutcome,
    rank: u32,
}

impl RankedOutcome {
    #[must_use]
    pub fn new(proposal: Proposal, outcome: ValidationOutcome, rank: u32) -> Self {
        Self {
            proposal,
            outcome,
            rank,
        }
    }

    #[must_use]
    pub fn proposal(&self) -> &Proposal {
        &self.proposal
    }

    #[must_use]
    pub fn outcome(&self) -> &ValidationOutcome {
        &self.outcome
    }

    #[must_use]
    pub fn rank(&self) -> u32 {
        self.rank
    }
}
