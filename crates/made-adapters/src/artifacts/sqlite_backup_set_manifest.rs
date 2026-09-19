use made_core::ports::ArtifactIdempotencyKey;
use made_core::value_objects::ArtifactDigest;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Evidence that the database snapshot and its exact external blobs verified.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SqliteBackupSetManifest {
    pub version: u32,
    pub protection_key: ArtifactIdempotencyKey,
    pub database_manifest_digest: ArtifactDigest,
    pub artifact_manifest_digest: ArtifactDigest,
    pub verified_at: OffsetDateTime,
}

impl SqliteBackupSetManifest {
    pub(super) const VERSION: u32 = 1;

    pub(super) fn validate(&self) -> bool {
        self.version == Self::VERSION
    }
}
