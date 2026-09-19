use made_core::ports::{ArtifactIdempotencyKey, ArtifactRecord};
use made_core::value_objects::ArtifactDigest;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Verifiable PostgreSQL custom archive containing state and in-database blobs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostgresBackupManifest {
    pub version: u32,
    pub protection_key: ArtifactIdempotencyKey,
    pub source_identity: ArtifactDigest,
    /// The exported snapshot token passed to `pg_dump --snapshot`.
    pub snapshot_id: String,
    /// The transaction snapshot observed while the export token was created.
    pub transaction_snapshot: String,
    /// Artifact metadata read from the same MVCC snapshot as the dump.
    pub artifact_records: Vec<ArtifactRecord>,
    pub archive_digest: ArtifactDigest,
    pub archive_bytes: u64,
    pub captured_at: OffsetDateTime,
}

impl PostgresBackupManifest {
    pub(super) const VERSION: u32 = 2;

    pub(super) fn validate(&self) -> bool {
        self.version == Self::VERSION
            && !self.snapshot_id.is_empty()
            && !self.transaction_snapshot.is_empty()
            && self.artifact_records.windows(2).all(|records| {
                records[0].artifact.artifact_id() < records[1].artifact.artifact_id()
            })
    }
}
