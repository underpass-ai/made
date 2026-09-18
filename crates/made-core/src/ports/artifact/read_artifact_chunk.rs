use crate::value_objects::ArtifactId;

use super::{ArtifactByteOffset, ArtifactChunkLimit};

/// One bounded artifact read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadArtifactChunk {
    pub artifact_id: ArtifactId,
    pub offset: ArtifactByteOffset,
    pub max_bytes: ArtifactChunkLimit,
}
