use crate::entities::{CeremonyAgentActivity, CeremonyAgentStatus};

/// Atomic roster snapshot and subsequent activity slice at one feed boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyAgentActivityPage {
    snapshot: Vec<CeremonyAgentStatus>,
    activities: Vec<CeremonyAgentActivity>,
    next_sequence: u64,
    head_sequence: u64,
    oldest_retained_sequence: Option<u64>,
    snapshot_complete: bool,
}

impl CeremonyAgentActivityPage {
    #[must_use]
    pub fn new(
        snapshot: Vec<CeremonyAgentStatus>,
        activities: Vec<CeremonyAgentActivity>,
        next_sequence: u64,
        head_sequence: u64,
        oldest_retained_sequence: Option<u64>,
        snapshot_complete: bool,
    ) -> Self {
        Self {
            snapshot,
            activities,
            next_sequence,
            head_sequence,
            oldest_retained_sequence,
            snapshot_complete,
        }
    }

    #[must_use]
    pub fn snapshot(&self) -> &[CeremonyAgentStatus] {
        &self.snapshot
    }
    #[must_use]
    pub fn activities(&self) -> &[CeremonyAgentActivity] {
        &self.activities
    }
    #[must_use]
    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }
    #[must_use]
    pub const fn head_sequence(&self) -> u64 {
        self.head_sequence
    }
    #[must_use]
    pub const fn oldest_retained_sequence(&self) -> Option<u64> {
        self.oldest_retained_sequence
    }
    #[must_use]
    pub const fn snapshot_complete(&self) -> bool {
        self.snapshot_complete
    }
}
