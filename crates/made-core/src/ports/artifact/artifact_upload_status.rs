use serde::{Deserialize, Serialize};

use super::{ArtifactByteOffset, ArtifactChunkLimit, ArtifactUploadId};

/// Durable progress returned by begin and chunk writes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactUploadStatus {
    pub upload_id: ArtifactUploadId,
    pub next_offset: ArtifactByteOffset,
    pub chunk_limit: ArtifactChunkLimit,
}
