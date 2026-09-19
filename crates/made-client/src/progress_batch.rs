use made_proto::v1::{CeremonyEventRecord, StreamCeremonyEndReason};

use crate::ProgressCheckpoint;

/// One bounded G6 replay/watch response and the cursor safe to persist.
#[derive(Clone, Debug)]
pub struct ProgressBatch {
    records: Vec<CeremonyEventRecord>,
    checkpoint: ProgressCheckpoint,
    head_sequence: u64,
    end_reason: StreamCeremonyEndReason,
}

impl ProgressBatch {
    pub(crate) fn new(
        records: Vec<CeremonyEventRecord>,
        checkpoint: ProgressCheckpoint,
        head_sequence: u64,
        end_reason: StreamCeremonyEndReason,
    ) -> Self {
        Self {
            records,
            checkpoint,
            head_sequence,
            end_reason,
        }
    }

    #[must_use]
    pub fn records(&self) -> &[CeremonyEventRecord] {
        &self.records
    }

    #[must_use]
    pub fn checkpoint(&self) -> &ProgressCheckpoint {
        &self.checkpoint
    }

    #[must_use]
    pub fn head_sequence(&self) -> u64 {
        self.head_sequence
    }

    #[must_use]
    pub fn end_reason(&self) -> StreamCeremonyEndReason {
        self.end_reason
    }
}
