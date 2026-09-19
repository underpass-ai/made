use serde::{Deserialize, Serialize};

use crate::value_objects::ArtifactRef;

use super::ArtifactTombstone;

/// Metadata and optional retirement audit state for one artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRecord {
    pub artifact: ArtifactRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tombstone: Option<ArtifactTombstone>,
}
