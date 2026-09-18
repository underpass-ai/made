use serde::{Deserialize, Serialize};

use crate::value_objects::ArtifactDigest;

/// Bytes returned by one bounded read plus their independent digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactChunkPage {
    pub bytes: Vec<u8>,
    pub next_offset: u64,
    pub chunk_digest: ArtifactDigest,
    pub eof: bool,
}
