use made_core::value_objects::{ArtifactDigest, ArtifactId};
use serde::{Deserialize, Serialize};

/// One content-addressed blob safe to remove after every reference retired.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactGcCandidate {
    pub digest: ArtifactDigest,
    pub bytes: u64,
    pub artifact_ids: Vec<ArtifactId>,
}
