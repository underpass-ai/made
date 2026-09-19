use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use made_core::ports::{
    ArtifactByteOffset, ArtifactChunkLimit, ArtifactIdempotencyKey, ArtifactPageLimit,
    ArtifactStoreError, ArtifactStorePort, BeginArtifactUpload, PutArtifactChunk,
    ReadArtifactChunk, TombstoneArtifact, ARTIFACT_DEFAULT_CHUNK_BYTES,
};
use made_core::value_objects::ArtifactDigest;

use super::artifact_backup_entry::ArtifactBackupEntry;
use super::artifact_backup_manifest::{
    ArtifactBackupManifest, ArtifactBackupPlan, ARTIFACT_BACKUP_VERSION,
};
use super::hashing::{digest_bytes, digest_reader, stable_key};
use super::local_artifact_io::write_json_atomic;

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

    /// Build the immutable, stable-order selection used by a backup.
    pub async fn plan(&self) -> Result<ArtifactBackupPlan, ArtifactStoreError> {
        let mut entries = Vec::new();
        let mut after = None;
        loop {
            let page = self
                .store
                .list(after.as_ref(), ArtifactPageLimit::default())
                .await?;
            entries.extend(
                page.items
                    .into_iter()
                    .map(|record| ArtifactBackupEntry { record }),
            );
            match page.next_after {
                Some(cursor) => after = Some(cursor),
                None => break,
            }
        }
        ArtifactBackupPlan::from_entries(entries)
            .map_err(|_| ArtifactStoreError::StorageUnavailable)
    }

    /// Prepare a resumable backup directory and persist its immutable plan.
    pub async fn prepare(
        &self,
        destination: impl AsRef<Path>,
    ) -> Result<ArtifactBackupPlan, ArtifactStoreError> {
        let root = destination.as_ref();
        fs::create_dir_all(root).map_err(storage_failure)?;
        fs::create_dir_all(root.join("blobs")).map_err(storage_failure)?;
        let manifest_path = root.join("manifest.json");
        if manifest_path.exists() {
            let manifest: ArtifactBackupManifest = read_json(&manifest_path)?;
            if !manifest.validate() {
                return Err(ArtifactStoreError::InvalidBackup);
            }
            return Ok(manifest.plan);
        }
        let plan = self.plan().await?;
        let manifest = ArtifactBackupManifest {
            version: ARTIFACT_BACKUP_VERSION,
            plan: plan.clone(),
            completed: Vec::new(),
            complete: false,
        };
        write_json_durable(&root.join("plan.json"), &plan)?;
        write_json_durable(&manifest_path, &manifest)?;
        sync_directory(root.join("blobs").as_path())?;
        sync_directory(root)?;
        Ok(plan)
    }

    /// Create or resume a deterministic local backup. Repeating a completed
    /// call verifies and returns success without rewriting it.
    pub async fn backup_to(&self, destination: impl AsRef<Path>) -> Result<(), ArtifactStoreError> {
        let destination = destination.as_ref();
        if destination.exists() && !destination.join("manifest.json").exists() {
            return Err(ArtifactStoreError::IdempotencyConflict);
        }
        let root = if destination.exists() {
            destination.to_path_buf()
        } else {
            let staging = staging_path(destination);
            fs::create_dir_all(&staging).map_err(storage_failure)?;
            staging
        };
        self.prepare(&root).await?;
        let result = self.resume_backup(&root).await;
        if result.is_err() && root != destination {
            // Keep the staging directory: a later invocation can resume it.
        }
        result?;
        if root != destination {
            fs::rename(&root, destination).map_err(storage_failure)?;
            sync_directory(destination.parent().unwrap_or_else(|| Path::new(".")))?;
        }
        Ok(())
    }

    pub fn inspect_manifest(
        source: impl AsRef<Path>,
    ) -> Result<ArtifactBackupManifest, ArtifactStoreError> {
        let manifest: ArtifactBackupManifest = read_json(&source.as_ref().join("manifest.json"))?;
        if !manifest.validate() || !manifest.complete {
            return Err(ArtifactStoreError::InvalidBackup);
        }
        for entry in &manifest.plan.entries {
            verify_backup_blob(source.as_ref(), entry)?;
        }
        Ok(manifest)
    }

    /// Verify the complete source before mutating the target, then restore it
    /// through the same resumable upload boundary used by normal writes.
    pub async fn restore_from(&self, source: impl AsRef<Path>) -> Result<(), ArtifactStoreError> {
        let source = source.as_ref();
        let manifest = Self::inspect_manifest(source)?;
        for entry in &manifest.plan.entries {
            self.validate_target(entry).await?;
        }
        for entry in &manifest.plan.entries {
            self.restore_entry(source, entry).await?;
        }
        Ok(())
    }

    async fn resume_backup(&self, root: &Path) -> Result<(), ArtifactStoreError> {
        let manifest_path = root.join("manifest.json");
        let mut manifest: ArtifactBackupManifest = read_json(&manifest_path)?;
        if !manifest.validate() {
            return Err(ArtifactStoreError::InvalidBackup);
        }
        if manifest.complete {
            for entry in &manifest.plan.entries {
                verify_backup_blob(root, entry)?;
            }
            return Ok(());
        }

        for entry in &manifest.plan.entries {
            if manifest
                .completed
                .iter()
                .any(|id| id == entry.artifact_id())
            {
                verify_backup_blob(root, entry)?;
                continue;
            }
            let path = blob_path(root, entry.artifact_id().as_str());
            write_backup_blob(self.store.as_ref(), &path, entry).await?;
            verify_blob(
                &path,
                entry.record.artifact.digest(),
                entry.record.artifact.size_bytes().get(),
            )?;
            manifest.completed.push(entry.artifact_id().clone());
            manifest.completed.sort();
            write_json_durable(&manifest_path, &manifest)?;
        }
        sync_directory(root.join("blobs").as_path())?;
        manifest.complete = true;
        write_json_durable(&manifest_path, &manifest)?;
        sync_directory(root)
    }

    async fn validate_target(&self, entry: &ArtifactBackupEntry) -> Result<(), ArtifactStoreError> {
        match self.store.get(entry.artifact_id()).await {
            Ok(existing) if existing.artifact == entry.record.artifact => {
                if let (Some(expected), Some(actual)) =
                    (&entry.record.tombstone, &existing.tombstone)
                {
                    if expected != actual {
                        return Err(ArtifactStoreError::IdempotencyConflict);
                    }
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

async fn write_backup_blob<S: ArtifactStorePort>(
    store: &S,
    path: &Path,
    entry: &ArtifactBackupEntry,
) -> Result<(), ArtifactStoreError> {
    let temporary = path.with_extension("part");
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temporary)
        .map_err(storage_failure)?;
    let mut offset = 0_u64;
    loop {
        let chunk = store
            .read_chunk_for_backup(ReadArtifactChunk {
                artifact_id: entry.artifact_id().clone(),
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
    fs::rename(&temporary, path).map_err(storage_failure)
}

fn verify_backup_blob(root: &Path, entry: &ArtifactBackupEntry) -> Result<(), ArtifactStoreError> {
    let reference = &entry.record.artifact;
    verify_blob(
        &blob_path(root, entry.artifact_id().as_str()),
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

fn staging_path(destination: &Path) -> PathBuf {
    destination.with_file_name(format!(
        ".{}-in-progress",
        destination
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("artifact-backup")
    ))
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, ArtifactStoreError> {
    let bytes = fs::read(path).map_err(|_| ArtifactStoreError::InvalidBackup)?;
    serde_json::from_slice(&bytes).map_err(|_| ArtifactStoreError::InvalidBackup)
}

fn write_json_durable(
    path: &Path,
    value: &impl serde::Serialize,
) -> Result<(), ArtifactStoreError> {
    write_json_atomic(path, value)?;
    File::open(path)
        .and_then(|file| file.sync_all())
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
