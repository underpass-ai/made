use serde::{Deserialize, Serialize};

/// Why a local process stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalExecutionOutcome {
    Completed,
    Failed,
    TimedOut,
    OutputLimitExceeded,
}
