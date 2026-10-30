use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolEndReason {
    Completed,
    Cancelled,
    ProtocolDeadline,
    StateDeadline,
}

impl ProtocolEndReason {
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::ProtocolDeadline => "protocol_deadline",
            Self::StateDeadline => "state_deadline",
        }
    }
}
