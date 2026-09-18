use serde::{Deserialize, Serialize};

use crate::value_objects::ArtifactDigest;

use super::{ArtifactByteOffset, ArtifactReadCompletion};

/// Bytes returned by one bounded read plus their independent digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactChunkPage {
    pub bytes: Vec<u8>,
    pub next_offset: ArtifactByteOffset,
    pub chunk_digest: ArtifactDigest,
    pub completion: ArtifactReadCompletion,
}

impl ArtifactChunkPage {
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.completion.is_complete()
    }
}
