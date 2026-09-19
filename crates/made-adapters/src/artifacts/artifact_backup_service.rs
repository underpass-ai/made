use std::fs::{self, File};
use std::io::{Read, Seek};
use std::path::Path;
use std::sync::Arc;

use made_core::ports::{
    ArtifactByteOffset, ArtifactIdempotencyKey, ArtifactPageLimit, ArtifactSnapshot,
    ArtifactStoreError, ArtifactStorePort, BeginArtifactUpload, PutArtifactChunk,
    TombstoneArtifact, ARTIFACT_DEFAULT_CHUNK_BYTES,
};

use super::artifact_backup_entry::ArtifactBackupEntry;
use super::artifact_backup_io::{
    backup_key, blob_path, read_json, staging_path, storage_failure, sync_directory,
    verify_backup_blob, verify_blob, write_backup_blob, write_json_durable,
};
use super::artifact_backup_manifest::{
    ArtifactBackupManifest, ArtifactBackupPlan, ARTIFACT_BACKUP_VERSION,
};
use super::hashing::digest_bytes;
use super::ArtifactBackupContent;

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

    /// Preview the current stable-order selection without protecting it from GC.
    /// Only [`Self::prepare`] establishes a durable backup protection.
    pub async fn plan(&self) -> Result<ArtifactBackupPlan, ArtifactStoreError> {
        let mut entries = Vec::new();
        let mut after = None;
        loop {
            let page = self
                .store
                .list(after.as_ref(), ArtifactPageLimit::default())
                .await?;
            for record in page.items {
                entries.push(self.backup_entry(record).await?);
            }
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
            if !manifest.complete {
                let key = manifest.protection_key.clone().unwrap_or(backup_key(root)?);
                self.store
                    .protect_references(
                        key,
                        manifest
                            .plan
                            .entries
                            .iter()
                            .map(|entry| entry.artifact_id().clone())
                            .collect(),
                    )
                    .await?;
            }
            return Ok(manifest.plan);
        }
        let key = backup_key(root)?;
        write_json_durable(&root.join("backup-owner.json"), &key)?;
        let snapshot = self.store.protect_snapshot(key.clone()).await?;
        let plan = ArtifactBackupPlan::from_entries(self.backup_entries(snapshot.records).await?)
            .map_err(|_| ArtifactStoreError::StorageUnavailable)?;
        let manifest = ArtifactBackupManifest {
            version: ARTIFACT_BACKUP_VERSION,
            plan: plan.clone(),
            completed: Vec::new(),
            complete: false,
            protection_key: Some(key),
        };
        write_json_durable(&root.join("plan.json"), &plan)?;
        write_json_durable(&manifest_path, &manifest)?;
        sync_directory(root.join("blobs").as_path())?;
        sync_directory(root)?;
        Ok(plan)
    }

    /// Persist the blob plan for an already protected database boundary.
    /// The caller owns release and must do it only after every component of
    /// the composed backup has been verified.
    pub async fn prepare_snapshot(
        &self,
        destination: impl AsRef<Path>,
        snapshot: &ArtifactSnapshot,
    ) -> Result<ArtifactBackupPlan, ArtifactStoreError> {
        if !snapshot.is_protected() {
            return Err(ArtifactStoreError::IdempotencyConflict);
        }
        let root = destination.as_ref();
        fs::create_dir_all(root.join("blobs")).map_err(storage_failure)?;
        let plan =
            ArtifactBackupPlan::from_entries(self.backup_entries(snapshot.records.clone()).await?)
                .map_err(|_| ArtifactStoreError::StorageUnavailable)?;
        let manifest_path = root.join("manifest.json");
        if manifest_path.exists() {
            let manifest: ArtifactBackupManifest = read_json(&manifest_path)?;
            if !manifest.validate()
                || manifest.plan != plan
                || manifest.protection_key.as_ref() != Some(&snapshot.key)
            {
                return Err(ArtifactStoreError::IdempotencyConflict);
            }
            return Ok(plan);
        }
        write_json_durable(&root.join("backup-owner.json"), &snapshot.key)?;
        write_json_durable(&root.join("plan.json"), &plan)?;
        write_json_durable(
            &manifest_path,
            &ArtifactBackupManifest {
                version: ARTIFACT_BACKUP_VERSION,
                plan: plan.clone(),
                completed: Vec::new(),
                complete: false,
                protection_key: Some(snapshot.key.clone()),
            },
        )?;
        sync_directory(root.join("blobs").as_path())?;
        sync_directory(root)?;
        Ok(plan)
    }

    /// Copy and verify exactly the supplied protected selection. The pin stays
    /// live so a database+blob coordinator can verify its other component.
    pub async fn backup_snapshot_to(
        &self,
        destination: impl AsRef<Path>,
        snapshot: &ArtifactSnapshot,
    ) -> Result<(), ArtifactStoreError> {
        let root = destination.as_ref();
        self.prepare_snapshot(root, snapshot).await?;
        self.resume_backup(root, false).await
    }

    async fn backup_entries(
        &self,
        records: Vec<made_core::ports::ArtifactRecord>,
    ) -> Result<Vec<ArtifactBackupEntry>, ArtifactStoreError> {
        let mut entries = Vec::with_capacity(records.len());
        for record in records {
            entries.push(self.backup_entry(record).await?);
        }
        Ok(entries)
    }

    async fn backup_entry(
        &self,
        record: made_core::ports::ArtifactRecord,
    ) -> Result<ArtifactBackupEntry, ArtifactStoreError> {
        let available = self
            .store
            .backup_content_available(record.artifact.artifact_id())
            .await?;
        let content = if available {
            ArtifactBackupContent::Present
        } else if record.tombstone.is_some() {
            ArtifactBackupContent::RetiredMetadataOnly
        } else {
            return Err(ArtifactStoreError::InvalidBackup);
        };
        Ok(ArtifactBackupEntry { record, content })
    }

    /// Create or resume a deterministic local backup. Repeating a completed
    /// call verifies and returns success without rewriting it.
    pub async fn backup_to(&self, destination: impl AsRef<Path>) -> Result<(), ArtifactStoreError> {
        let destination = destination.as_ref();
        if destination.exists()
            && !destination.join("manifest.json").exists()
            && !destination.join("backup-owner.json").exists()
        {
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
        let result = self.resume_backup(&root, true).await;
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

    /// Explicit administrative abandonment; a released identity cannot be reused.
    pub async fn abandon(&self, destination: impl AsRef<Path>) -> Result<(), ArtifactStoreError> {
        let root = destination.as_ref();
        let key = match read_json::<ArtifactBackupManifest>(&root.join("manifest.json")) {
            Ok(manifest) => manifest
                .protection_key
                .ok_or(ArtifactStoreError::InvalidBackup)?,
            Err(_) => backup_key(root)?,
        };
        self.store.release_snapshot(&key).await
    }

    pub fn inspect_manifest(
        source: impl AsRef<Path>,
    ) -> Result<ArtifactBackupManifest, ArtifactStoreError> {
        let manifest: ArtifactBackupManifest = read_json(&source.as_ref().join("manifest.json"))?;
        if !manifest.validate() || !manifest.complete {
            return Err(ArtifactStoreError::InvalidBackup);
        }
        for entry in &manifest.plan.entries {
            if entry.content == ArtifactBackupContent::Present {
                verify_backup_blob(source.as_ref(), entry)?;
            }
        }
        Ok(manifest)
    }

    /// Verify the complete source before mutating the target, then restore it
    /// through the same resumable upload boundary used by normal writes.
    pub async fn restore_from(&self, source: impl AsRef<Path>) -> Result<(), ArtifactStoreError> {
        let source = source.as_ref();
        self.restore_from_protected(source).await?;
        self.finish_restore(source).await
    }

    /// Restore while retaining the temporary restore protection so a composed
    /// database restore can recreate its durable receipt protections first.
    pub async fn restore_from_protected(
        &self,
        source: impl AsRef<Path>,
    ) -> Result<(), ArtifactStoreError> {
        let source = source.as_ref();
        let manifest = Self::inspect_manifest(source)?;
        self.store
            .protect_restore(
                ArtifactIdempotencyKey::new(format!("restore:{}", manifest.plan.plan_digest))?,
                manifest
                    .plan
                    .entries
                    .iter()
                    .map(|entry| entry.record.clone())
                    .collect(),
            )
            .await?;
        for entry in &manifest.plan.entries {
            self.validate_target(entry).await?;
        }
        for entry in &manifest.plan.entries {
            self.restore_entry(source, entry).await?;
        }
        for entry in &manifest.plan.entries {
            self.validate_target(entry).await?;
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

    pub async fn finish_restore(&self, source: impl AsRef<Path>) -> Result<(), ArtifactStoreError> {
        let manifest = Self::inspect_manifest(source)?;
        self.store
            .release_snapshot(&ArtifactIdempotencyKey::new(format!(
                "restore:{}",
                manifest.plan.plan_digest
            ))?)
            .await
    }

    async fn resume_backup(
        &self,
        root: &Path,
        release_after: bool,
    ) -> Result<(), ArtifactStoreError> {
        let manifest_path = root.join("manifest.json");
        let mut manifest: ArtifactBackupManifest = read_json(&manifest_path)?;
        if !manifest.validate() {
            return Err(ArtifactStoreError::InvalidBackup);
        }
        if manifest.complete {
            for entry in &manifest.plan.entries {
                if entry.content == ArtifactBackupContent::Present {
                    verify_backup_blob(root, entry)?;
                }
            }
            if release_after {
                if let Some(key) = &manifest.protection_key {
                    self.store.release_snapshot(key).await?;
                }
            }
            return Ok(());
        }

        // Reopen validates the durable pin, including the crash window between
        // pin creation and manifest persistence. Legacy interrupted backups are
        // protected before any further source reads.
        let key = manifest.protection_key.clone().unwrap_or(backup_key(root)?);
        let snapshot = self
            .store
            .protect_references(
                key.clone(),
                manifest
                    .plan
                    .entries
                    .iter()
                    .map(|entry| entry.artifact_id().clone())
                    .collect(),
            )
            .await?;
        if snapshot.records.len() != manifest.plan.entries.len()
            || snapshot
                .records
                .iter()
                .zip(&manifest.plan.entries)
                .any(|(record, entry)| record != &entry.record)
        {
            return Err(ArtifactStoreError::InvalidBackup);
        }
        manifest.protection_key = Some(key.clone());

        for entry in &manifest.plan.entries {
            if entry.content == ArtifactBackupContent::RetiredMetadataOnly {
                if !manifest
                    .completed
                    .iter()
                    .any(|id| id == entry.artifact_id())
                {
                    manifest.completed.push(entry.artifact_id().clone());
                    manifest.completed.sort();
                    write_json_durable(&manifest_path, &manifest)?;
                }
                continue;
            }
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
        sync_directory(root)?;
        if release_after {
            self.store.release_snapshot(&key).await
        } else {
            Ok(())
        }
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
