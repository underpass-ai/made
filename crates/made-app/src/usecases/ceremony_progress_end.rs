use made_core::value_objects::StreamVersion;

use super::CeremonyProgressEndReason;

/// Final resume cursor and stop reason for one successful stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyProgressEnd {
    resume_after_sequence: StreamVersion,
    head_sequence: StreamVersion,
    reason: CeremonyProgressEndReason,
    resume_after_activity_sequence: u64,
    activity_head_sequence: u64,
}

impl CeremonyProgressEnd {
    #[must_use]
    pub const fn new(
        resume_after_sequence: StreamVersion,
        head_sequence: StreamVersion,
        reason: CeremonyProgressEndReason,
    ) -> Self {
        Self {
            resume_after_sequence,
            head_sequence,
            reason,
            resume_after_activity_sequence: 0,
            activity_head_sequence: 0,
        }
    }

    #[must_use]
    pub const fn with_activity(
        mut self,
        resume_after_activity_sequence: u64,
        activity_head_sequence: u64,
    ) -> Self {
        self.resume_after_activity_sequence = resume_after_activity_sequence;
        self.activity_head_sequence = activity_head_sequence;
        self
    }

    #[must_use]
    pub const fn resume_after_sequence(&self) -> StreamVersion {
        self.resume_after_sequence
    }

    #[must_use]
    pub const fn head_sequence(&self) -> StreamVersion {
        self.head_sequence
    }

    #[must_use]
    pub const fn reason(&self) -> CeremonyProgressEndReason {
        self.reason
    }
    #[must_use]
    pub const fn resume_after_activity_sequence(&self) -> u64 {
        self.resume_after_activity_sequence
    }
    #[must_use]
    pub const fn activity_head_sequence(&self) -> u64 {
        self.activity_head_sequence
    }
}
