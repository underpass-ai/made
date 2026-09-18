use crate::value_objects::ArtifactId;

/// One bounded artifact read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadArtifactChunk {
    pub artifact_id: ArtifactId,
    pub offset: u64,
    pub max_bytes: u32,
}
