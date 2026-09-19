use made_core::value_objects::{ArtifactDigest, ArtifactId};
use serde::{Deserialize, Serialize};

use super::ArtifactGcExclusionReason;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactGcExclusion {
    pub digest: ArtifactDigest,
    pub artifact_ids: Vec<ArtifactId>,
    pub reasons: Vec<ArtifactGcExclusionReason>,
}
