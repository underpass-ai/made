use made_core::value_objects::CeremonyId;

use super::{CeremonyWorkClaimFailure, CeremonyWorkerBatchOutcome};

/// One discovery page with claim failures and the drained worker batch.
#[derive(Debug, Clone, PartialEq)]
pub struct CeremonyWorkerHostOutcome {
    batch: CeremonyWorkerBatchOutcome,
    claim_failures: Vec<CeremonyWorkClaimFailure>,
    next_cursor: Option<CeremonyId>,
}

impl CeremonyWorkerHostOutcome {
    #[must_use]
    pub const fn new(
        batch: CeremonyWorkerBatchOutcome,
        claim_failures: Vec<CeremonyWorkClaimFailure>,
        next_cursor: Option<CeremonyId>,
    ) -> Self {
        Self {
            batch,
            claim_failures,
            next_cursor,
        }
    }

    #[must_use]
    pub const fn batch(&self) -> &CeremonyWorkerBatchOutcome {
        &self.batch
    }

    #[must_use]
    pub fn claim_failures(&self) -> &[CeremonyWorkClaimFailure] {
        &self.claim_failures
    }

    #[must_use]
    pub const fn next_cursor(&self) -> Option<&CeremonyId> {
        self.next_cursor.as_ref()
    }
}
