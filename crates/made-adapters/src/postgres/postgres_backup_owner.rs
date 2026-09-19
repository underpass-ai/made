use made_core::ports::{ArtifactIdempotencyKey, ArtifactRecord};
use made_core::value_objects::ArtifactDigest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct PostgresBackupOwner {
    pub(super) protection_key: ArtifactIdempotencyKey,
    pub(super) source_identity: ArtifactDigest,
    pub(super) snapshot_id: String,
    pub(super) transaction_snapshot: String,
    pub(super) artifact_records: Vec<ArtifactRecord>,
}
