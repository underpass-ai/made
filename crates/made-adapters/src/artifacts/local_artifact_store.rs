use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use made_core::ports::{
    ArtifactChunkPage, ArtifactPage, ArtifactPageLimit, ArtifactRecord, ArtifactStoreError,
    ArtifactStorePort, ArtifactTombstone, ArtifactUploadId, ArtifactUploadStatus,
    BeginArtifactUpload, PutArtifactChunk, ReadArtifactChunk, TombstoneArtifact,
};
use made_core::value_objects::{ArtifactId, ArtifactRef, AuthorizationEvidence};
use time::OffsetDateTime;

use super::local_artifact_repository::LocalArtifactRepository;
use super::{ArtifactGcPlan, ArtifactGcReport};

/// Durable single-host artifact store with cross-process serialization.
#[derive(Debug, Clone)]
pub struct LocalArtifactStore {
    repository: Arc<LocalArtifactRepository>,
}

impl LocalArtifactStore {
    pub async fn store_identity(
        &self,
    ) -> Result<made_core::value_objects::ArtifactDigest, ArtifactStoreError> {
        self.blocking(LocalArtifactRepository::store_identity).await
    }
    /// Pin the artifact selection and capture a database snapshot while the
    /// artifact barrier is still held. The callback must not re-enter this
    /// artifact store or acquire a database write lock.
    ///
    /// Persisting the pin before the callback is deliberate: a process crash
    /// during database capture leaves a conservative orphan pin instead of a
    /// database snapshot whose blobs can be collected before resume.
    pub async fn protect_snapshot_and_then<F>(
        &self,
        key: made_core::ports::ArtifactIdempotencyKey,
        capture_database: F,
    ) -> Result<made_core::ports::ArtifactSnapshot, ArtifactStoreError>
    where
        F: FnOnce() -> Result<(), ArtifactStoreError> + Send + 'static,
    {
        self.blocking(move |repository| {
            repository.locked(|| {
                let snapshot = repository.protect_locked(key, None)?;
                capture_database()?;
                Ok(snapshot)
            })
        })
        .await
    }

    pub async fn protect_snapshot_bundle_and_then<F>(
        &self,
        key: made_core::ports::ArtifactIdempotencyKey,
        capture_database: F,
    ) -> Result<
        (
            made_core::ports::ArtifactSnapshot,
            Vec<made_core::ports::ArtifactSnapshot>,
        ),
        ArtifactStoreError,
    >
    where
        F: FnOnce() -> Result<(), ArtifactStoreError> + Send + 'static,
    {
        self.blocking(move |repository| {
            repository.locked(|| {
                let snapshot = repository.protect_locked(key.clone(), None)?;
                capture_database()?;
                let protections = repository
                    .active_protections_locked()?
                    .into_iter()
                    .filter(|protection| protection.key != key)
                    .collect();
                Ok((snapshot, protections))
            })
        })
        .await
    }
    pub fn open(root: impl AsRef<Path>) -> Result<Self, ArtifactStoreError> {
        Ok(Self {
            repository: Arc::new(LocalArtifactRepository::open(root)?),
        })
    }

    pub async fn plan_gc(
        &self,
        retire_before: OffsetDateTime,
        lease: made_core::value_objects::StepLease,
    ) -> Result<ArtifactGcPlan, ArtifactStoreError> {
        self.blocking(move |repository| repository.plan_gc(retire_before, lease))
            .await
    }

    pub async fn apply_gc(
        &self,
        plan: &ArtifactGcPlan,
        now: OffsetDateTime,
    ) -> Result<ArtifactGcReport, ArtifactStoreError> {
        let plan = plan.clone();
        self.blocking(move |repository| repository.apply_gc(&plan, now))
            .await
    }

    #[must_use]
    pub fn dry_run_gc(plan: &ArtifactGcPlan) -> ArtifactGcReport {
        ArtifactGcReport::dry_run(plan)
    }

    pub async fn plan_garbage_collection(
        &self,
        retire_before: OffsetDateTime,
        lease: made_core::value_objects::StepLease,
    ) -> Result<ArtifactGcPlan, ArtifactStoreError> {
        self.plan_gc(retire_before, lease).await
    }

    pub async fn apply_garbage_collection(
        &self,
        plan: &ArtifactGcPlan,
        now: OffsetDateTime,
    ) -> Result<ArtifactGcReport, ArtifactStoreError> {
        self.apply_gc(plan, now).await
    }

    #[must_use]
    pub fn dry_run_garbage_collection(plan: &ArtifactGcPlan) -> ArtifactGcReport {
        Self::dry_run_gc(plan)
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
                ArtifactStoreError::unavailable_static("local artifact blocking operation failed")
            })?
    }
}

#[async_trait]
impl ArtifactStorePort for LocalArtifactStore {
    async fn protect_restore(
        &self,
        key: made_core::ports::RestoreProtectionKey,
        records: Vec<ArtifactRecord>,
    ) -> Result<made_core::ports::ArtifactSnapshot, ArtifactStoreError> {
        self.blocking(move |repository| {
            repository.protect_restore(key.into_idempotency_key(), records)
        })
        .await
    }
    async fn protect_snapshot(
        &self,
        key: made_core::ports::ArtifactIdempotencyKey,
    ) -> Result<made_core::ports::ArtifactSnapshot, ArtifactStoreError> {
        self.blocking(move |repository| repository.protect(key, None))
            .await
    }

    async fn protect_references(
        &self,
        key: made_core::ports::ArtifactIdempotencyKey,
        ids: Vec<ArtifactId>,
    ) -> Result<made_core::ports::ArtifactSnapshot, ArtifactStoreError> {
        self.blocking(move |repository| repository.protect(key, Some(ids)))
            .await
    }

    async fn release_snapshot(
        &self,
        key: &made_core::ports::ArtifactIdempotencyKey,
    ) -> Result<(), ArtifactStoreError> {
        let key = key.clone();
        self.blocking(move |repository| repository.release_protection(&key))
            .await
    }
    async fn release_restore(
        &self,
        key: &made_core::ports::RestoreProtectionKey,
    ) -> Result<(), ArtifactStoreError> {
        let key = key.as_idempotency_key().clone();
        self.blocking(move |repository| repository.release_protection(&key))
            .await
    }
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

    async fn commit_upload_authorized(
        &self,
        upload_id: &ArtifactUploadId,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<ArtifactRef, ArtifactStoreError> {
        let upload_id = upload_id.clone();
        self.blocking(move |repository| repository.commit_authorized(&upload_id, authorization))
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

    async fn backup_content_available(
        &self,
        artifact_id: &made_core::value_objects::ArtifactId,
    ) -> Result<bool, ArtifactStoreError> {
        let repository = self.repository.clone();
        let artifact_id = artifact_id.clone();
        tokio::task::spawn_blocking(move || {
            let record = repository.load_record(&artifact_id)?;
            match std::fs::File::open(repository.layout.blob(&record.artifact)) {
                Ok(file) => {
                    super::local_artifact_repository::verify_reader(file, &record.artifact)?;
                    Ok(true)
                }
                Err(error)
                    if error.kind() == std::io::ErrorKind::NotFound
                        && record.tombstone.is_some() =>
                {
                    Ok(false)
                }
                Err(error) => Err(super::local_artifact_io::storage_failure(error)),
            }
        })
        .await
        .map_err(|error| ArtifactStoreError::unavailable("serialize or decode artifact metadata", error))?
    }

    async fn restore_retired_metadata(
        &self,
        record: ArtifactRecord,
    ) -> Result<(), ArtifactStoreError> {
        self.blocking(move |repository| repository.restore_retired_metadata(&record))
            .await
    }

    async fn active_protections(
        &self,
    ) -> Result<Vec<made_core::ports::ArtifactSnapshot>, ArtifactStoreError> {
        self.blocking(LocalArtifactRepository::active_protections)
            .await
    }

    async fn tombstone(
        &self,
        command: TombstoneArtifact,
    ) -> Result<ArtifactTombstone, ArtifactStoreError> {
        self.blocking(move |repository| repository.tombstone(command))
            .await
    }

    async fn tombstone_authorized(
        &self,
        command: TombstoneArtifact,
        authorization: Option<AuthorizationEvidence>,
    ) -> Result<ArtifactTombstone, ArtifactStoreError> {
        self.blocking(move |repository| repository.tombstone_authorized(command, authorization))
            .await
    }
}
