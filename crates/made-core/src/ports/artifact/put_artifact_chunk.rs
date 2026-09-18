use serde::{Deserialize, Serialize};

use crate::value_objects::ArtifactDigest;

use super::ArtifactUploadId;

/// One bounded, independently verified upload write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PutArtifactChunk {
    pub upload_id: ArtifactUploadId,
    pub offset: u64,
    pub bytes: Vec<u8>,
    pub chunk_digest: ArtifactDigest,
}
