use made_proto::v1::{
    ProtocolAgentActivity, ProtocolAgentActivitySnapshot, ProtocolEventRecord,
    StreamProtocolEndReason,
};

use crate::ProgressCheckpoint;

/// One bounded combined engine/host progress read and both resume cursors.
#[derive(Clone, Debug)]
pub struct AgentProgressBatch {
    records: Vec<ProtocolEventRecord>,
    snapshot: Option<ProtocolAgentActivitySnapshot>,
    activities: Vec<ProtocolAgentActivity>,
    checkpoint: ProgressCheckpoint,
    activity_checkpoint: u64,
    event_head_sequence: u64,
    activity_head_sequence: u64,
    end_reason: StreamProtocolEndReason,
}

impl AgentProgressBatch {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        records: Vec<ProtocolEventRecord>,
        snapshot: Option<ProtocolAgentActivitySnapshot>,
        activities: Vec<ProtocolAgentActivity>,
        checkpoint: ProgressCheckpoint,
        activity_checkpoint: u64,
        event_head_sequence: u64,
        activity_head_sequence: u64,
        end_reason: StreamProtocolEndReason,
    ) -> Self {
        Self {
            records,
            snapshot,
            activities,
            checkpoint,
            activity_checkpoint,
            event_head_sequence,
            activity_head_sequence,
            end_reason,
        }
    }

    #[must_use]
    pub fn records(&self) -> &[ProtocolEventRecord] {
        &self.records
    }
    #[must_use]
    pub const fn snapshot(&self) -> Option<&ProtocolAgentActivitySnapshot> {
        self.snapshot.as_ref()
    }
    #[must_use]
    pub fn activities(&self) -> &[ProtocolAgentActivity] {
        &self.activities
    }
    #[must_use]
    pub const fn checkpoint(&self) -> &ProgressCheckpoint {
        &self.checkpoint
    }
    #[must_use]
    pub const fn activity_checkpoint(&self) -> u64 {
        self.activity_checkpoint
    }
    #[must_use]
    pub const fn event_head_sequence(&self) -> u64 {
        self.event_head_sequence
    }
    #[must_use]
    pub const fn activity_head_sequence(&self) -> u64 {
        self.activity_head_sequence
    }
    #[must_use]
    pub const fn end_reason(&self) -> StreamProtocolEndReason {
        self.end_reason
    }
}
