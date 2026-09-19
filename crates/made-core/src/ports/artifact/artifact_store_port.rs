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
        _authorization: Option<AuthorizationEvidence>,
    ) -> Result<ArtifactRef, ArtifactStoreError> {
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
        _authorization: Option<AuthorizationEvidence>,
    ) -> Result<ArtifactTombstone, ArtifactStoreError> {
        self.tombstone(command).await
    }
}
