use serde::{Deserialize, Serialize};

/// A stable failure classification sealed with a step's result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepFailureKind {
    NoValidProposal,
    Timeout,
}

impl StepFailureKind {
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::NoValidProposal => "no_valid_proposal",
            Self::Timeout => "timeout",
        }
    }
}
