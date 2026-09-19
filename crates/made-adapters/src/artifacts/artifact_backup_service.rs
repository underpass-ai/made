use std::fs::{self, File};
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use made_core::ports::{
    ArtifactByteOffset, ArtifactChunkLimit, ArtifactIdempotencyKey, ArtifactPageLimit,
    ArtifactStoreError, ArtifactStorePort, BeginArtifactUpload, PutArtifactChunk,
    ReadArtifactChunk, TombstoneArtifact, ARTIFACT_DEFAULT_CHUNK_BYTES,
};
use made_core::value_objects::ArtifactDigest;
use uuid::Uuid;

use super::artifact_backup_entry::ArtifactBackupEntry;
use super::artifact_backup_manifest::ArtifactBackupManifest;
use super::hashing::{digest_bytes, digest_reader, stable_key};

/// Filesystem backup/restore composed only from bounded artifact-store calls.
#[derive(Debug)]
pub struct ArtifactBackupService<S> {
    store: Arc<S>,
}

impl<S> ArtifactBackupService<S>
where
    S: ArtifactStorePort + 'static,
{
    #[must_use]
    pub fn new(store: Arc<S>) -> Self {
        Self { store }
    }

    pub async fn backup_to(&self, destination: impl AsRef<Path>) -> Result<(), ArtifactStoreError> {
        let destination = destination.as_ref();
        if destination.exists() {
            return Err(ArtifactStoreError::IdempotencyConflict);
        }
        let parent = destination
            .parent()
            .ok_or(ArtifactStoreError::InvalidBackup)?;
        fs::create_dir_all(parent).map_err(storage_failure)?;
        let temporary = parent.join(format!(".artifact-backup-{}", Uuid::new_v4()));
        fs::create_dir(&temporary).map_err(storage_failure)?;
        fs::create_dir(temporary.join("blobs")).map_err(storage_failure)?;

        let result = self.write_backup(&temporary).await;
        if let Err(error) = result {
            let _ = fs::remove_dir_all(&temporary);
            return Err(error);
        }
        fs::rename(&temporary, destination).map_err(storage_failure)?;
        sync_directory(parent)
    }

    pub async fn restore_from(&self, source: impl AsRef<Path>) -> Result<(), ArtifactStoreError> {
        let source = source.as_ref();
        let manifest: ArtifactBackupManifest = read_json(&source.join("manifest.json"))?;
        if manifest.version != 1 {
            return Err(ArtifactStoreError::InvalidBackup);
        }

        for entry in &manifest.entries {
            verify_backup_blob(source, entry)?;
        }
        for entry in manifest.entries {
            self.restore_entry(source, entry).await?;
        }
        Ok(())
    }

    async fn write_backup(&self, root: &Path) -> Result<(), ArtifactStoreError> {
        let mut entries = Vec::new();
        let mut after = None;
        loop {
            let page = self
                .store
                .list(after.as_ref(), ArtifactPageLimit::default())
                .await?;
            for record in page.items {
                let path = blob_path(root, record.artifact.artifact_id().as_str());
                let mut file = File::create(&path).map_err(storage_failure)?;
                let mut offset = 0_u64;
                loop {
                    let chunk = self
                        .store
                        .read_chunk_for_backup(ReadArtifactChunk {
                            artifact_id: record.artifact.artifact_id().clone(),
                            offset: ArtifactByteOffset::new(offset),
                            max_bytes: ArtifactChunkLimit::DEFAULT,
                        })
                        .await?;
                    if digest_bytes(&chunk.bytes) != chunk.chunk_digest {
                        return Err(ArtifactStoreError::InvalidBackup);
                    }
                    file.write_all(&chunk.bytes).map_err(storage_failure)?;
                    offset = chunk.next_offset.get();
                    if chunk.is_complete() {
                        break;
                    }
                }
                file.sync_all().map_err(storage_failure)?;
                verify_blob(
                    &path,
                    record.artifact.digest(),
                    record.artifact.size_bytes().get(),
                )?;
                entries.push(ArtifactBackupEntry { record });
            }
            match page.next_after {
                Some(cursor) => after = Some(cursor),
                None => break,
            }
        }
        write_json_durable(
            &root.join("manifest.json"),
            &ArtifactBackupManifest {
                version: 1,
                entries,
            },
        )?;
        sync_directory(&root.join("blobs"))?;
        sync_directory(root)
    }

    async fn restore_entry(
        &self,
        root: &Path,
        entry: ArtifactBackupEntry,
    ) -> Result<(), ArtifactStoreError> {
        let reference = &entry.record.artifact;
        let artifact_authorization = entry.record.authorization.clone();
        let key = ArtifactIdempotencyKey::new(format!(
            "backup-v1:{}:{}",
            reference.artifact_id(),
            reference.digest()
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
            offset = status.next_offset.get();
        }
        let restored = self
            .store
            .commit_upload_authorized(&upload.upload_id, artifact_authorization)
            .await?;
        if restored != *reference {
            return Err(ArtifactStoreError::InvalidBackup);
        }
        if let Some(tombstone) = entry.record.tombstone {
            let authorization = tombstone.authorization.clone();
            self.store
                .tombstone_authorized(
                    TombstoneArtifact {
                        artifact_id: reference.artifact_id().clone(),
                        actor: tombstone.actor,
                        policy: tombstone.policy,
                        retired_at: tombstone.retired_at,
                    },
                    authorization,
                )
                .await?;
        }
        Ok(())
    }
}

fn verify_backup_blob(root: &Path, entry: &ArtifactBackupEntry) -> Result<(), ArtifactStoreError> {
    let reference = &entry.record.artifact;
    verify_blob(
        &blob_path(root, reference.artifact_id().as_str()),
        reference.digest(),
        reference.size_bytes().get(),
    )
}

fn verify_blob(
    path: &Path,
    expected_digest: &ArtifactDigest,
    expected_size: u64,
) -> Result<(), ArtifactStoreError> {
    let file = File::open(path).map_err(|_| ArtifactStoreError::InvalidBackup)?;
    let (digest, size) = digest_reader(file).map_err(|_| ArtifactStoreError::InvalidBackup)?;
    if &digest != expected_digest || size != expected_size {
        return Err(ArtifactStoreError::InvalidBackup);
    }
    Ok(())
}

fn blob_path(root: &Path, artifact_id: &str) -> PathBuf {
    root.join("blobs").join(stable_key(artifact_id))
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, ArtifactStoreError> {
    let bytes = fs::read(path).map_err(|_| ArtifactStoreError::InvalidBackup)?;
    serde_json::from_slice(&bytes).map_err(|_| ArtifactStoreError::InvalidBackup)
}

fn write_json_durable(
    path: &Path,
    value: &impl serde::Serialize,
) -> Result<(), ArtifactStoreError> {
    let bytes =
        serde_json::to_vec_pretty(value).map_err(|_| ArtifactStoreError::StorageUnavailable)?;
    let mut file = File::create(path).map_err(storage_failure)?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(storage_failure)
}

fn sync_directory(path: &Path) -> Result<(), ArtifactStoreError> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(storage_failure)
}

fn storage_failure(error: std::io::Error) -> ArtifactStoreError {
    tracing::error!(%error, "artifact backup operation failed");
    drop(error);
    ArtifactStoreError::StorageUnavailable
}
