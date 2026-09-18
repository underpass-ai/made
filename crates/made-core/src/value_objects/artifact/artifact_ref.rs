use serde::{Deserialize, Serialize};

use super::{ArtifactDigest, ArtifactId, ArtifactMediaType, ArtifactProvenance, ArtifactSizeBytes};
use crate::error::DomainError;

/// Verifiable artifact metadata. It never contains storage coordinates or bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRef {
    artifact_id: ArtifactId,
    digest: ArtifactDigest,
    size_bytes: ArtifactSizeBytes,
    media_type: ArtifactMediaType,
    provenance: ArtifactProvenance,
}

impl ArtifactRef {
    #[must_use]
    pub const fn new(
        artifact_id: ArtifactId,
        digest: ArtifactDigest,
        size_bytes: ArtifactSizeBytes,
        media_type: ArtifactMediaType,
        provenance: ArtifactProvenance,
    ) -> Self {
        Self {
            artifact_id,
            digest,
            size_bytes,
            media_type,
            provenance,
        }
    }

    #[must_use]
    pub const fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    #[must_use]
    pub const fn digest(&self) -> &ArtifactDigest {
        &self.digest
    }

    #[must_use]
    pub const fn size_bytes(&self) -> ArtifactSizeBytes {
        self.size_bytes
    }

    #[must_use]
    pub const fn media_type(&self) -> &ArtifactMediaType {
        &self.media_type
    }

    #[must_use]
    pub const fn provenance(&self) -> &ArtifactProvenance {
        &self.provenance
    }

    /// Re-check nested provenance after deserialization.
    pub fn validate(&self) -> Result<(), DomainError> {
        self.provenance.validate()
    }
}
