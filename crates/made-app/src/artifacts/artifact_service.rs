use std::sync::Arc;

use made_core::ports::{
    ArtifactChunkPage, ArtifactIdempotencyKey, ArtifactPage, ArtifactPageLimit, ArtifactRecord,
    ArtifactStoreError, ArtifactStorePort, ArtifactTombstone, ArtifactUploadId,
    ArtifactUploadStatus, BeginArtifactUpload, PutArtifactChunk, ReadArtifactChunk,
    TombstoneArtifact, ARTIFACT_DEFAULT_CHUNK_BYTES,
};
use made_core::value_objects::{
    ArtifactDigest, ArtifactId, ArtifactMediaType, ArtifactProvenance, ArtifactRef,
    ArtifactSizeBytes,
};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

/// Public application facade for bounded artifact operations.
pub struct ArtifactService {
    store: Arc<dyn ArtifactStorePort>,
}

impl std::fmt::Debug for ArtifactService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("ArtifactService").finish()
    }
}

impl ArtifactService {
    #[must_use]
    pub fn new(store: Arc<dyn ArtifactStorePort>) -> Self {
        Self { store }
    }

    pub async fn begin_upload(
        &self,
        request: BeginArtifactUpload,
    ) -> Result<ArtifactUploadStatus, ArtifactStoreError> {
        self.store.begin_upload(request).await
    }

    pub async fn put_chunk(
        &self,
        request: PutArtifactChunk,
    ) -> Result<ArtifactUploadStatus, ArtifactStoreError> {
        self.store.put_chunk(request).await
    }

    pub async fn commit_upload(
        &self,
        upload_id: &ArtifactUploadId,
    ) -> Result<ArtifactRef, ArtifactStoreError> {
        self.store.commit_upload(upload_id).await
    }

    pub async fn abort_upload(
        &self,
        upload_id: &ArtifactUploadId,
    ) -> Result<(), ArtifactStoreError> {
        self.store.abort_upload(upload_id).await
    }

    pub async fn get(
        &self,
        artifact_id: &ArtifactId,
    ) -> Result<ArtifactRecord, ArtifactStoreError> {
        self.store.get(artifact_id).await
    }

    pub async fn list(
        &self,
        after: Option<&ArtifactId>,
        limit: ArtifactPageLimit,
    ) -> Result<ArtifactPage, ArtifactStoreError> {
        self.store.list(after, limit).await
    }

    pub async fn read_chunk(
        &self,
        request: ReadArtifactChunk,
    ) -> Result<ArtifactChunkPage, ArtifactStoreError> {
        self.store.read_chunk(request).await
    }

    /// The host must authorize this command before crossing this C5.6 boundary.
    pub async fn tombstone(
        &self,
        command: TombstoneArtifact,
    ) -> Result<ArtifactTombstone, ArtifactStoreError> {
        self.store.tombstone(command).await
    }

    /// Persist a generated report through the same verified chunk protocol.
    pub async fn save_generated_report(
        &self,
        bytes: &[u8],
        media_type: ArtifactMediaType,
        observed_at: OffsetDateTime,
        idempotency_key: ArtifactIdempotencyKey,
    ) -> Result<ArtifactRef, ArtifactStoreError> {
        let expected_digest = digest(bytes);
        let status = self
            .begin_upload(BeginArtifactUpload {
                requested_artifact_id: None,
                expected_digest,
                size_bytes: ArtifactSizeBytes::new(bytes.len() as u64),
                media_type,
                provenance: ArtifactProvenance::generated_report(observed_at),
                idempotency_key,
            })
            .await?;
        let mut offset = status.next_offset;
        while offset < bytes.len() as u64 {
            let end = (offset as usize + ARTIFACT_DEFAULT_CHUNK_BYTES as usize).min(bytes.len());
            let chunk = bytes[offset as usize..end].to_vec();
            let progress = self
                .put_chunk(PutArtifactChunk {
                    upload_id: status.upload_id.clone(),
                    offset,
                    chunk_digest: digest(&chunk),
                    bytes: chunk,
                })
                .await?;
            offset = progress.next_offset;
        }
        self.commit_upload(&status.upload_id).await
    }
}

fn digest(bytes: &[u8]) -> ArtifactDigest {
    ArtifactDigest::new(format!("sha256:{:x}", Sha256::digest(bytes)))
        .expect("sha256 formatting is canonical")
}
