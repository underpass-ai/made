use serde::{Deserialize, Serialize};

use crate::value_objects::ArtifactDigest;

use super::{ArtifactByteOffset, ArtifactUploadId};

/// One bounded, independently verified upload write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PutArtifactChunk {
    pub upload_id: ArtifactUploadId,
    pub offset: ArtifactByteOffset,
    pub bytes: Vec<u8>,
    pub chunk_digest: ArtifactDigest,
}
