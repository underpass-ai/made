use made_core::value_objects::ProtocolId;

use super::{ProtocolWorkClaimFailure, ProtocolWorkerBatchOutcome};

/// One discovery page with claim failures and the drained worker batch.
#[derive(Debug, Clone, PartialEq)]
pub struct ProtocolWorkerHostOutcome {
    batch: ProtocolWorkerBatchOutcome,
    claim_failures: Vec<ProtocolWorkClaimFailure>,
    next_cursor: Option<ProtocolId>,
}

impl ProtocolWorkerHostOutcome {
    #[must_use]
    pub const fn new(
        batch: ProtocolWorkerBatchOutcome,
        claim_failures: Vec<ProtocolWorkClaimFailure>,
        next_cursor: Option<ProtocolId>,
    ) -> Self {
        Self {
            batch,
            claim_failures,
            next_cursor,
        }
    }

    #[must_use]
    pub const fn batch(&self) -> &ProtocolWorkerBatchOutcome {
        &self.batch
    }

    #[must_use]
    pub fn claim_failures(&self) -> &[ProtocolWorkClaimFailure] {
        &self.claim_failures
    }

    #[must_use]
    pub const fn next_cursor(&self) -> Option<&ProtocolId> {
        self.next_cursor.as_ref()
    }
}
