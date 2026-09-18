use serde::{Deserialize, Serialize};

use crate::value_objects::{
    ArtifactDigest, ArtifactId, ArtifactMediaType, ArtifactProvenance, ArtifactSizeBytes,
};

use super::ArtifactIdempotencyKey;

/// Validated metadata supplied before any artifact bytes are admitted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeginArtifactUpload {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_artifact_id: Option<ArtifactId>,
    pub expected_digest: ArtifactDigest,
    pub size_bytes: ArtifactSizeBytes,
    pub media_type: ArtifactMediaType,
    pub provenance: ArtifactProvenance,
    pub idempotency_key: ArtifactIdempotencyKey,
}
