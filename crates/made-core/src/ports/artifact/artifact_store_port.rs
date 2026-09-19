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
