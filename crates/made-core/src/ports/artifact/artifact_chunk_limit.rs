use serde::{Deserialize, Serialize};

use crate::DomainError;

use super::{ARTIFACT_DEFAULT_CHUNK_BYTES, ARTIFACT_MAX_CHUNK_BYTES};

/// Validated bound for one artifact transfer operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ArtifactChunkLimit(u32);

impl ArtifactChunkLimit {
    pub const DEFAULT: Self = Self(ARTIFACT_DEFAULT_CHUNK_BYTES);
    pub const MAX: Self = Self(ARTIFACT_MAX_CHUNK_BYTES);

    pub fn new(value: u32) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "artifact_chunk_limit",
            });
        }
        if value > ARTIFACT_MAX_CHUNK_BYTES {
            return Err(DomainError::OutOfRange {
                field: "artifact_chunk_limit",
                value: f64::from(value),
                min: 1.0,
                max: f64::from(ARTIFACT_MAX_CHUNK_BYTES),
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl Default for ArtifactChunkLimit {
    fn default() -> Self {
        Self::DEFAULT
    }
}
