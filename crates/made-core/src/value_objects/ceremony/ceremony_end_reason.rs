use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CeremonyEndReason {
    Completed,
    Cancelled,
    CeremonyDeadline,
    StateDeadline,
}

impl CeremonyEndReason {
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::CeremonyDeadline => "ceremony_deadline",
            Self::StateDeadline => "state_deadline",
        }
    }
}
