use serde::{Deserialize, Serialize};

/// Freshness of the host observation, deliberately separate from execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentLiveness {
    Fresh,
    Stale,
    Unreachable,
    Unknown,
}
