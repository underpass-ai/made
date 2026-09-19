use made_core::ports::ArtifactIdempotencyKey;
use made_core::value_objects::ArtifactDigest;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Verifiable PostgreSQL custom archive containing state and in-database blobs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostgresBackupManifest {
    pub version: u32,
    pub protection_key: ArtifactIdempotencyKey,
    pub source_identity: ArtifactDigest,
    pub archive_digest: ArtifactDigest,
    pub archive_bytes: u64,
    pub captured_at: OffsetDateTime,
}

impl PostgresBackupManifest {
    pub(super) const VERSION: u32 = 1;

    pub(super) fn validate(&self) -> bool {
        self.version == Self::VERSION
    }
}
