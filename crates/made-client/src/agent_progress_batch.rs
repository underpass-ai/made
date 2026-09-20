use made_proto::v1::{
    CeremonyAgentActivity, CeremonyAgentActivitySnapshot, CeremonyEventRecord,
    StreamCeremonyEndReason,
};

use crate::ProgressCheckpoint;

/// One bounded combined engine/host progress read and both resume cursors.
#[derive(Clone, Debug)]
pub struct AgentProgressBatch {
    records: Vec<CeremonyEventRecord>,
    snapshot: Option<CeremonyAgentActivitySnapshot>,
    activities: Vec<CeremonyAgentActivity>,
    checkpoint: ProgressCheckpoint,
    activity_checkpoint: u64,
    event_head_sequence: u64,
    activity_head_sequence: u64,
    end_reason: StreamCeremonyEndReason,
}

impl AgentProgressBatch {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        records: Vec<CeremonyEventRecord>,
        snapshot: Option<CeremonyAgentActivitySnapshot>,
        activities: Vec<CeremonyAgentActivity>,
        checkpoint: ProgressCheckpoint,
        activity_checkpoint: u64,
        event_head_sequence: u64,
        activity_head_sequence: u64,
        end_reason: StreamCeremonyEndReason,
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
    pub fn records(&self) -> &[CeremonyEventRecord] {
        &self.records
    }
    #[must_use]
    pub const fn snapshot(&self) -> Option<&CeremonyAgentActivitySnapshot> {
        self.snapshot.as_ref()
    }
    #[must_use]
    pub fn activities(&self) -> &[CeremonyAgentActivity] {
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
    pub const fn end_reason(&self) -> StreamCeremonyEndReason {
        self.end_reason
    }
}
