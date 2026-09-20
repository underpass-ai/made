use made_core::entities::CeremonyAgentStatus;

/// Current filtered roster captured at an activity-feed boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyAgentSnapshot {
    agents: Vec<CeremonyAgentStatus>,
    activity_head_sequence: u64,
    complete: bool,
}

impl CeremonyAgentSnapshot {
    #[must_use]
    pub fn new(
        agents: Vec<CeremonyAgentStatus>,
        activity_head_sequence: u64,
        complete: bool,
    ) -> Self {
        Self {
            agents,
            activity_head_sequence,
            complete,
        }
    }

    #[must_use]
    pub fn agents(&self) -> &[CeremonyAgentStatus] {
        &self.agents
    }

    #[must_use]
    pub const fn activity_head_sequence(&self) -> u64 {
        self.activity_head_sequence
    }
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.complete
    }
}
