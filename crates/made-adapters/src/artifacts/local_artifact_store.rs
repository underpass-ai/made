use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use made_core::ports::{
    ArtifactChunkPage, ArtifactPage, ArtifactPageLimit, ArtifactRecord, ArtifactStoreError,
    ArtifactStorePort, ArtifactTombstone, ArtifactUploadId, ArtifactUploadStatus,
    BeginArtifactUpload, PutArtifactChunk, ReadArtifactChunk, TombstoneArtifact,
};
use made_core::value_objects::{ArtifactId, ArtifactRef};

use super::local_artifact_repository::LocalArtifactRepository;

/// Durable single-host artifact store with cross-process serialization.
#[derive(Debug, Clone)]
pub struct LocalArtifactStore {
    repository: Arc<LocalArtifactRepository>,
}

impl LocalArtifactStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, ArtifactStoreError> {
        Ok(Self {
            repository: Arc::new(LocalArtifactRepository::open(root)?),
        })
    }

    async fn blocking<T: Send + 'static>(
        &self,
        operation: impl FnOnce(&LocalArtifactRepository) -> Result<T, ArtifactStoreError>
            + Send
            + 'static,
    ) -> Result<T, ArtifactStoreError> {
        let repository = self.repository.clone();
        tokio::task::spawn_blocking(move || operation(&repository))
            .await
            .map_err(|error| {
                tracing::error!(%error, "local artifact blocking operation failed");
                ArtifactStoreError::StorageUnavailable
            })?
    }
}

#[async_trait]
impl ArtifactStorePort for LocalArtifactStore {
    async fn begin_upload(
        &self,
        request: BeginArtifactUpload,
    ) -> Result<ArtifactUploadStatus, ArtifactStoreError> {
        self.blocking(move |repository| repository.begin(request))
            .await
    }

    async fn put_chunk(
        &self,
        request: PutArtifactChunk,
    ) -> Result<ArtifactUploadStatus, ArtifactStoreError> {
        self.blocking(move |repository| repository.put(&request))
            .await
    }

    async fn artifact_id_for_upload(
        &self,
        upload_id: &ArtifactUploadId,
    ) -> Result<ArtifactId, ArtifactStoreError> {
        let upload_id = upload_id.clone();
        self.blocking(move |repository| repository.artifact_id_for_upload(&upload_id))
            .await
    }

    async fn commit_upload(
        &self,
        upload_id: &ArtifactUploadId,
    ) -> Result<ArtifactRef, ArtifactStoreError> {
        let upload_id = upload_id.clone();
        self.blocking(move |repository| repository.commit(&upload_id))
            .await
    }

    async fn abort_upload(&self, upload_id: &ArtifactUploadId) -> Result<(), ArtifactStoreError> {
        let upload_id = upload_id.clone();
        self.blocking(move |repository| repository.abort(&upload_id))
            .await
    }

    async fn get(&self, artifact_id: &ArtifactId) -> Result<ArtifactRecord, ArtifactStoreError> {
        let artifact_id = artifact_id.clone();
        self.blocking(move |repository| repository.get(&artifact_id))
            .await
    }

    async fn list(
        &self,
        after: Option<&ArtifactId>,
        limit: ArtifactPageLimit,
    ) -> Result<ArtifactPage, ArtifactStoreError> {
        let after = after.cloned();
        self.blocking(move |repository| repository.list(after.as_ref(), limit))
            .await
    }

    async fn read_chunk(
        &self,
        request: ReadArtifactChunk,
    ) -> Result<ArtifactChunkPage, ArtifactStoreError> {
        self.blocking(move |repository| repository.read(&request, false))
            .await
    }

    async fn read_chunk_for_backup(
        &self,
        request: ReadArtifactChunk,
    ) -> Result<ArtifactChunkPage, ArtifactStoreError> {
        self.blocking(move |repository| repository.read(&request, true))
            .await
    }

    async fn tombstone(
        &self,
        command: TombstoneArtifact,
    ) -> Result<ArtifactTombstone, ArtifactStoreError> {
        self.blocking(move |repository| repository.tombstone(command))
            .await
    }
}
