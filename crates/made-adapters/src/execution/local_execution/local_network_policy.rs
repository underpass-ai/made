use serde::{Deserialize, Serialize};

/// Network declaration for a trusted-local invocation, not an OS sandbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalNetworkPolicy {
    #[serde(rename = "allowed")]
    Allowed,
    #[serde(rename = "not-allowed")]
    NotAllowed,
}
