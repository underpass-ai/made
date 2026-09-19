use serde::{Deserialize, Serialize};

/// Execution state reported by a host worker, independent of liveness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentExecutionStatus {
    Running,
    Waiting,
    Blocked,
    Finished,
}
