use made_proto::v1::{ProtocolEventRecord, StreamProtocolEndReason};

use crate::ProgressCheckpoint;

/// One bounded G6 replay/watch response and the cursor safe to persist.
#[derive(Clone, Debug)]
pub struct ProgressBatch {
    records: Vec<ProtocolEventRecord>,
    checkpoint: ProgressCheckpoint,
    head_sequence: u64,
    end_reason: StreamProtocolEndReason,
}

impl ProgressBatch {
    pub(crate) fn new(
        records: Vec<ProtocolEventRecord>,
        checkpoint: ProgressCheckpoint,
        head_sequence: u64,
        end_reason: StreamProtocolEndReason,
    ) -> Self {
        Self {
            records,
            checkpoint,
            head_sequence,
            end_reason,
        }
    }

    #[must_use]
    pub fn records(&self) -> &[ProtocolEventRecord] {
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
    pub fn end_reason(&self) -> StreamProtocolEndReason {
        self.end_reason
    }
}
