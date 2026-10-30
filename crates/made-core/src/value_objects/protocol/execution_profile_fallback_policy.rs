use serde::{Deserialize, Serialize};

/// Host behavior when the requested execution profile is unavailable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionProfileFallbackPolicy {
    Reject,
    Fallback,
}
