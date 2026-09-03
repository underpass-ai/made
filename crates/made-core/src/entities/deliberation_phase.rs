use serde::{Deserialize, Serialize};

/// Lifecycle phase of a deliberation aggregate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeliberationPhase {
    Proposing,
    Revising,
    Validating,
    Scoring,
    Completed,
}

impl DeliberationPhase {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Proposing => "Proposing",
            Self::Revising => "Revising",
            Self::Validating => "Validating",
            Self::Scoring => "Scoring",
            Self::Completed => "Completed",
        }
    }

    pub(super) fn next(self) -> Option<Self> {
        Some(match self {
            Self::Proposing => Self::Revising,
            Self::Revising => Self::Validating,
            Self::Validating => Self::Scoring,
            Self::Scoring => Self::Completed,
            Self::Completed => return None,
        })
    }
}
