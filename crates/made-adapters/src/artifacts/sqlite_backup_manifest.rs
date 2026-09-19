use made_core::ports::{ArtifactIdempotencyKey, ArtifactRecord};
use made_core::value_objects::ArtifactDigest;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Verifiable SQLite snapshot paired with the artifact selection pinned at capture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SqliteBackupManifest {
    pub version: u32,
    pub protection_key: ArtifactIdempotencyKey,
    pub source_identity: ArtifactDigest,
    pub artifact_store_identity: ArtifactDigest,
    pub database_digest: ArtifactDigest,
    pub database_bytes: u64,
    pub artifact_records_digest: ArtifactDigest,
    pub artifact_records: Vec<ArtifactRecord>,
    #[serde(default)]
    pub protections: Vec<made_core::ports::ArtifactSnapshot>,
    pub captured_at: OffsetDateTime,
}

impl SqliteBackupManifest {
    pub(super) const VERSION: u32 = 1;

    pub(super) fn validate(&self) -> bool {
        self.version == Self::VERSION
            && self.artifact_records.windows(2).all(|records| {
                records[0].artifact.artifact_id() < records[1].artifact.artifact_id()
            })
            && serde_json::to_vec(&self.artifact_records).is_ok_and(|bytes| {
                super::hashing::digest_bytes(&bytes) == self.artifact_records_digest
            })
    }
}
