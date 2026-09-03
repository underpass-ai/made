use serde::{Deserialize, Serialize};

/// Final domain status of an execution attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Succeeded,
    Failed,
}

impl ExecutionStatus {
    #[must_use]
    pub const fn is_success(self) -> bool {
        matches!(self, Self::Succeeded)
    }
}
