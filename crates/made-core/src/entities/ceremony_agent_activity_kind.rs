use serde::{Deserialize, Serialize};

/// Public, bounded progress classification emitted from a host status report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CeremonyAgentActivityKind {
    ActivityChanged,
    BlockerOpened,
    BlockerResolved,
    CheckpointAvailable,
    InterventionDelivered,
    InterventionAnswered,
    InputRequested,
    AgentReplaced,
    Stale,
    Completed,
    Failed,
    Heartbeat,
}

impl CeremonyAgentActivityKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ActivityChanged => "activity_changed",
            Self::BlockerOpened => "blocker_opened",
            Self::BlockerResolved => "blocker_resolved",
            Self::CheckpointAvailable => "checkpoint_available",
            Self::InterventionDelivered => "intervention_delivered",
            Self::InterventionAnswered => "intervention_answered",
            Self::InputRequested => "input_requested",
            Self::AgentReplaced => "agent_replaced",
            Self::Stale => "stale",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Heartbeat => "heartbeat",
        }
    }
}
