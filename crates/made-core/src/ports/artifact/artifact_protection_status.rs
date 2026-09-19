use serde::{Deserialize, Serialize};

/// Durable lifecycle of one artifact protection identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactProtectionStatus {
    Protected,
    Released,
}
