use std::fs::File;
use std::io::{Read, Seek};
use std::path::Path;
use std::sync::Arc;

use made_core::ports::{
    ArtifactByteOffset, ArtifactIdempotencyKey, ArtifactStoreError, ArtifactStorePort,
    BeginArtifactUpload, PutArtifactChunk, RestoreProtectionKey, TombstoneArtifact,
    ARTIFACT_DEFAULT_CHUNK_BYTES,
};

use super::artifact_backup_entry::ArtifactBackupEntry;
use super::artifact_backup_io::{blob_path, storage_failure};
use super::artifact_backup_manifest::ArtifactBackupManifest;
use super::artifact_backup_verification::inspect_backup_manifest;
use super::hashing::digest_bytes;
use super::ArtifactBackupContent;

/// Restores one verified artifact backup while fencing garbage collection.
#[derive(Debug)]
pub(super) struct ArtifactRestoreUseCase<S> {
    store: Arc<S>,
}

impl<S> ArtifactRestoreUseCase<S>
where
    S: ArtifactStorePort + 'static,
{
    pub(super) fn new(store: Arc<S>) -> Self {
        Self { store }
    }

    pub(super) async fn restore_from(
        &self,
        source: impl AsRef<Path>,
    ) -> Result<(), ArtifactStoreError> {
        let source = source.as_ref();
        self.restore_from_protected(source).await?;
        self.finish_restore(source).await
    }

    pub(super) async fn restore_from_protected(
        &self,
        source: impl AsRef<Path>,
    ) -> Result<(), ArtifactStoreError> {
        let source = source.as_ref();
        let manifest = inspect_backup_manifest(source)?;
        let snapshot = self
            .store
            .protect_restore(
                restore_key(&manifest)?,
                manifest
                    .plan
                    .entries
                    .iter()
                    .map(|entry| entry.record.clone())
                    .collect(),
            )
            .await?;
        if snapshot.is_released() {
            return self.verify_restored(&manifest).await;
        }
        for entry in &manifest.plan.entries {
            self.validate_target(entry).await?;
        }
        for entry in &manifest.plan.entries {
            self.restore_entry(source, entry).await?;
        }
        self.verify_restored(&manifest).await
    }

    pub(super) async fn finish_restore(
        &self,
        source: impl AsRef<Path>,
    ) -> Result<(), ArtifactStoreError> {
        let manifest = inspect_backup_manifest(source.as_ref())?;
        self.verify_restored(&manifest).await?;
        self.store.release_restore(&restore_key(&manifest)?).await
    }

    async fn verify_restored(
        &self,
        manifest: &ArtifactBackupManifest,
    ) -> Result<(), ArtifactStoreError> {
        for entry in &manifest.plan.entries {
            let existing = self.store.get(entry.artifact_id()).await?;
            if existing.artifact != entry.record.artifact
                || existing.tombstone != entry.record.tombstone
            {
                return Err(ArtifactStoreError::IdempotencyConflict);
            }
            if entry.content == ArtifactBackupContent::Present
                && !self
                    .store
                    .backup_content_available(entry.artifact_id())
                    .await?
            {
                return Err(ArtifactStoreError::InvalidBackup);
            }
        }
        Ok(())
    }

    async fn validate_target(&self, entry: &ArtifactBackupEntry) -> Result<(), ArtifactStoreError> {
        match self.store.get(entry.artifact_id()).await {
            Ok(existing) if existing.artifact == entry.record.artifact => {
                if existing.tombstone != entry.record.tombstone {
                    return Err(ArtifactStoreError::IdempotencyConflict);
                }
                Ok(())
            }
            Ok(_) => Err(ArtifactStoreError::IdempotencyConflict),
            Err(ArtifactStoreError::NotFound) => Ok(()),
            Err(error) => Err(error),
        }
    }

    async fn restore_entry(
        &self,
        root: &Path,
        entry: &ArtifactBackupEntry,
    ) -> Result<(), ArtifactStoreError> {
        if entry.content == ArtifactBackupContent::RetiredMetadataOnly {
            return self
                .store
                .restore_retired_metadata(entry.record.clone())
                .await;
        }
        let reference = &entry.record.artifact;
        let key = ArtifactIdempotencyKey::new(format!(
            "backup-v2:{}:{}:{}",
            reference.artifact_id(),
            reference.digest(),
            digest_bytes(reference.artifact_id().as_str().as_bytes())
        ))?;
        let upload = self
            .store
            .begin_upload(BeginArtifactUpload {
                requested_artifact_id: Some(reference.artifact_id().clone()),
                expected_digest: reference.digest().clone(),
                size_bytes: reference.size_bytes(),
                media_type: reference.media_type().clone(),
                provenance: reference.provenance().clone(),
                idempotency_key: key,
            })
            .await?;
        let mut file = File::open(blob_path(root, reference.artifact_id().as_str()))
            .map_err(storage_failure)?;
        if upload.next_offset.get() > reference.size_bytes().get() {
            return Err(ArtifactStoreError::InvalidBackup);
        }
        let mut offset = upload.next_offset.get();
        file.seek(std::io::SeekFrom::Start(offset))
            .map_err(storage_failure)?;
        let mut bytes = vec![0; ARTIFACT_DEFAULT_CHUNK_BYTES as usize];
        loop {
            let count = file.read(&mut bytes).map_err(storage_failure)?;
            if count == 0 {
                break;
            }
            let chunk = bytes[..count].to_vec();
            let status = self
                .store
                .put_chunk(PutArtifactChunk {
                    upload_id: upload.upload_id.clone(),
                    offset: ArtifactByteOffset::new(offset),
                    chunk_digest: digest_bytes(&chunk),
                    bytes: chunk,
                })
                .await?;
            if status.next_offset.get() <= offset {
                return Err(ArtifactStoreError::InvalidBackup);
            }
            offset = status.next_offset.get();
        }
        let restored = self
            .store
            .commit_upload_authorized(&upload.upload_id, entry.record.authorization.clone())
            .await?;
        if restored != *reference {
            return Err(ArtifactStoreError::InvalidBackup);
        }
        if let Some(tombstone) = &entry.record.tombstone {
            self.store
                .tombstone_authorized(
                    TombstoneArtifact {
                        artifact_id: reference.artifact_id().clone(),
                        actor: tombstone.actor.clone(),
                        policy: tombstone.policy.clone(),
                        retired_at: tombstone.retired_at,
                    },
                    tombstone.authorization.clone(),
                )
                .await?;
        }
        Ok(())
    }
}

fn restore_key(
    manifest: &ArtifactBackupManifest,
) -> Result<RestoreProtectionKey, ArtifactStoreError> {
    Ok(RestoreProtectionKey::for_plan_digest(
        manifest.plan.plan_digest.as_str(),
    )?)
}
