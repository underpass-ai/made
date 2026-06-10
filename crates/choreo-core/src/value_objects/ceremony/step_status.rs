use serde::{Deserialize, Serialize};

use crate::error::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StepStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    WaitingForHuman,
    Cancelled,
}

impl StepStatus {
    #[must_use]
    pub fn is_executable(self) -> bool {
        matches!(self, Self::Pending | Self::Failed | Self::WaitingForHuman)
    }

    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    #[must_use]
    pub fn is_success(self) -> bool {
        self == Self::Completed
    }

    /// Stable, low-cardinality label value for metrics exposition.
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::WaitingForHuman => "waiting_for_human",
            Self::Cancelled => "cancelled",
        }
    }
}

impl TryFrom<&str> for StepStatus {
    type Error = DomainError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "PENDING" => Ok(Self::Pending),
            "IN_PROGRESS" => Ok(Self::InProgress),
            "COMPLETED" => Ok(Self::Completed),
            "FAILED" => Ok(Self::Failed),
            "WAITING_FOR_HUMAN" => Ok(Self::WaitingForHuman),
            "CANCELLED" => Ok(Self::Cancelled),
            _ => Err(DomainError::InvariantViolated {
                reason: "unknown ceremony step status",
            }),
        }
    }
}
