use made_core::value_objects::StreamVersion;

use super::CeremonyProgressEndReason;

/// Final resume cursor and stop reason for one successful stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyProgressEnd {
    resume_after_sequence: StreamVersion,
    head_sequence: StreamVersion,
    reason: CeremonyProgressEndReason,
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
        }
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
}
