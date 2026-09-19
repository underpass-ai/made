use serde::{Deserialize, Serialize};

/// Runtime mode declared by the local execution adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalExecutionMode {
    #[serde(rename = "trusted-local")]
    TrustedLocal,
    #[serde(rename = "isolated-linux-unavailable")]
    IsolatedLinuxUnavailable,
}
