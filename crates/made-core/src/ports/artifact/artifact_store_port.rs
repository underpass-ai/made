use async_trait::async_trait;

use crate::value_objects::{ArtifactId, ArtifactRef, AuthorizationEvidence};

use super::{
    ArtifactChunkPage, ArtifactPage, ArtifactPageLimit, ArtifactRecord, ArtifactStoreError,
    ArtifactTombstone, ArtifactUploadId, ArtifactUploadStatus, BeginArtifactUpload,
    PutArtifactChunk, ReadArtifactChunk, TombstoneArtifact,
};

/// Durable, bounded artifact transfer and metadata boundary.
#[async_trait]
pub trait ArtifactStorePort: Send + Sync {
    /// Atomically select and protect the immutable store-side backup snapshot.
    /// Unsupported stores fail closed rather than offer an unsafe backup.
    async fn protect_snapshot(
        &self,
        _key: super::ArtifactIdempotencyKey,
    ) -> Result<super::ArtifactSnapshot, ArtifactStoreError> {
        Err(ArtifactStoreError::AccessDenied)
    }
    /// Protect exact references before publishing a receipt or restoring metadata.
    async fn protect_references(
        &self,
        _key: super::ArtifactIdempotencyKey,
        _ids: Vec<ArtifactId>,
    ) -> Result<super::ArtifactSnapshot, ArtifactStoreError> {
        Err(ArtifactStoreError::AccessDenied)
    }
    /// Administrative release after verified copy or explicit abandonment only.
    async fn release_snapshot(
        &self,
        _key: &super::ArtifactIdempotencyKey,
    ) -> Result<(), ArtifactStoreError> {
        Err(ArtifactStoreError::AccessDenied)
    }
    /// Release only the temporary protection owned by a verified restore.
    /// Receipt and other durable reference pins are never released through
    /// this capability.
    async fn release_restore(
        &self,
        _key: &super::RestoreProtectionKey,
    ) -> Result<(), ArtifactStoreError> {
        Err(ArtifactStoreError::AccessDenied)
    }
    /// Pin expected restore content before uploads/metadata become visible.
    async fn protect_restore(
        &self,
        _key: super::RestoreProtectionKey,
        _records: Vec<ArtifactRecord>,
    ) -> Result<super::ArtifactSnapshot, ArtifactStoreError> {
        Err(ArtifactStoreError::AccessDenied)
    }
    async fn begin_upload(
        &self,
        request: BeginArtifactUpload,
    ) -> Result<ArtifactUploadStatus, ArtifactStoreError>;
    async fn put_chunk(
        &self,
        request: PutArtifactChunk,
    ) -> Result<ArtifactUploadStatus, ArtifactStoreError>;
    /// Resolve an upload to the artifact identity sealed in its durable manifest.
    async fn artifact_id_for_upload(
        &self,
        upload_id: &ArtifactUploadId,
    ) -> Result<ArtifactId, ArtifactStoreError>;
    async fn commit_upload(
        &self,
        upload_id: &ArtifactUploadId,
    ) -> Result<ArtifactRef, ArtifactStoreError>;
    async fn commit_upload_authorized(
        &self,
        upload_id: &ArtifactUploadId,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<ArtifactRef, ArtifactStoreError> {
        reject_unsupported_authorization(authorization.as_ref())?;
        self.commit_upload(upload_id).await
    }
    async fn abort_upload(&self, upload_id: &ArtifactUploadId) -> Result<(), ArtifactStoreError>;
    async fn get(&self, artifact_id: &ArtifactId) -> Result<ArtifactRecord, ArtifactStoreError>;
    async fn list(
        &self,
        after: Option<&ArtifactId>,
        limit: ArtifactPageLimit,
    ) -> Result<ArtifactPage, ArtifactStoreError>;
    async fn read_chunk(
        &self,
        request: ReadArtifactChunk,
    ) -> Result<ArtifactChunkPage, ArtifactStoreError>;
    /// Administrative read used by verified backup. Public clients never receive this capability.
    async fn read_chunk_for_backup(
        &self,
        request: ReadArtifactChunk,
    ) -> Result<ArtifactChunkPage, ArtifactStoreError>;
    /// Verify digest and size of immutable bytes when present. Returns `false`
    /// only for retired metadata after authorized GC; corruption is an error.
    async fn backup_content_available(
        &self,
        _artifact_id: &ArtifactId,
    ) -> Result<bool, ArtifactStoreError> {
        Err(ArtifactStoreError::AccessDenied)
    }
    /// Recreate a retired metadata record whose content was already collected.
    async fn restore_retired_metadata(
        &self,
        _record: ArtifactRecord,
    ) -> Result<(), ArtifactStoreError> {
        Err(ArtifactStoreError::AccessDenied)
    }
    /// Durable protections that must survive a full store restore.
    async fn active_protections(&self) -> Result<Vec<super::ArtifactSnapshot>, ArtifactStoreError> {
        Err(ArtifactStoreError::AccessDenied)
    }
    async fn tombstone(
        &self,
        command: TombstoneArtifact,
    ) -> Result<ArtifactTombstone, ArtifactStoreError>;
    async fn tombstone_authorized(
        &self,
        command: TombstoneArtifact,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<ArtifactTombstone, ArtifactStoreError> {
        reject_unsupported_authorization(authorization.as_ref())?;
        self.tombstone(command).await
    }
}

fn reject_unsupported_authorization(
    authorization: Option<&AuthorizationEvidence>,
) -> Result<(), ArtifactStoreError> {
    if authorization.is_some() {
        return Err(ArtifactStoreError::AccessDenied);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_objects::AuthorizationEvidence;

    #[test]
    fn legacy_adapter_fallback_refuses_to_drop_authorization_evidence() {
        let evidence: AuthorizationEvidence = serde_json::from_value(serde_json::json!({
            "decision_id": "a".repeat(64),
            "request_id": "artifact-fallback",
            "principal_id": "artifact-owner",
            "action": "commit_artifact_upload",
            "scope": {"kind":"global"},
            "target_digest": "b".repeat(64),
            "policy_version": 1,
            "admitted_at": "2026-09-19T12:00:00Z",
            "valid_until": "2026-09-19T12:01:00Z"
        }))
        .unwrap();

        assert_eq!(
            reject_unsupported_authorization(Some(&evidence)),
            Err(ArtifactStoreError::AccessDenied)
        );
        assert_eq!(reject_unsupported_authorization(None), Ok(()));
    }
}
