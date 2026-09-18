use serde::{Deserialize, Serialize};

use super::ArtifactUploadId;

/// Durable progress returned by begin and chunk writes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactUploadStatus {
    pub upload_id: ArtifactUploadId,
    pub next_offset: u64,
    pub chunk_limit_bytes: u32,
}
