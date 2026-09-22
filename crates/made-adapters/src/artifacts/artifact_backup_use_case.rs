use std::fs;
use std::path::Path;
use std::sync::Arc;

use made_core::ports::{
    ArtifactPageLimit, ArtifactSnapshot, ArtifactStoreError, ArtifactStorePort,
};

use super::artifact_backup_entry::ArtifactBackupEntry;
use super::artifact_backup_io::{
    backup_content_failure, backup_key, blob_path, read_json, staging_path, storage_failure,
    sync_directory, verify_backup_blob, verify_blob, write_backup_blob, write_json_durable,
};
use super::artifact_backup_manifest::{
    ArtifactBackupManifest, ArtifactBackupPlan, ARTIFACT_BACKUP_VERSION,
};
use super::ArtifactBackupContent;

/// Creates and resumes immutable artifact backup sets.
#[derive(Debug)]
pub(super) struct ArtifactBackupUseCase<S> {
    store: Arc<S>,
}

impl<S> ArtifactBackupUseCase<S>
where
    S: ArtifactStorePort + 'static,
{
    pub(super) fn new(store: Arc<S>) -> Self {
        Self { store }
    }

    pub(super) async fn plan(&self) -> Result<ArtifactBackupPlan, ArtifactStoreError> {
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
        ArtifactBackupPlan::from_entries(entries).map_err(|error| {
            ArtifactStoreError::unavailable("serialize or decode artifact metadata", error)
        })
    }

    pub(super) async fn prepare(
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
                    .await
                    .map_err(backup_content_failure)?;
            }
            return Ok(manifest.plan);
        }
        let key = backup_key(root)?;
        write_json_durable(&root.join("backup-owner.json"), &key)?;
        let snapshot = self
            .store
            .protect_snapshot(key.clone())
            .await
            .map_err(backup_content_failure)?;
        let plan = ArtifactBackupPlan::from_entries(self.backup_entries(snapshot.records).await?)
            .map_err(|error| {
            ArtifactStoreError::unavailable("serialize or decode artifact metadata", error)
        })?;
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

    pub(super) async fn prepare_snapshot(
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
                .map_err(|error| {
                    ArtifactStoreError::unavailable("serialize or decode artifact metadata", error)
                })?;
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

    pub(super) async fn backup_snapshot_to(
        &self,
        destination: impl AsRef<Path>,
        snapshot: &ArtifactSnapshot,
    ) -> Result<(), ArtifactStoreError> {
        let root = destination.as_ref();
        self.prepare_snapshot(root, snapshot).await?;
        self.resume_backup(root, false).await
    }

    pub(super) async fn backup_to(
        &self,
        destination: impl AsRef<Path>,
    ) -> Result<(), ArtifactStoreError> {
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
        self.resume_backup(&root, true).await?;
        if root != destination {
            fs::rename(&root, destination).map_err(storage_failure)?;
            sync_directory(destination.parent().unwrap_or_else(|| Path::new(".")))?;
        }
        Ok(())
    }

    pub(super) async fn abandon(
        &self,
        destination: impl AsRef<Path>,
    ) -> Result<(), ArtifactStoreError> {
        let root = destination.as_ref();
        let key = match read_json::<ArtifactBackupManifest>(&root.join("manifest.json")) {
            Ok(manifest) => manifest
                .protection_key
                .ok_or(ArtifactStoreError::InvalidBackup)?,
            Err(_) => backup_key(root)?,
        };
        self.store.release_snapshot(&key).await
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
            .await
            .map_err(backup_content_failure)?;
        let content = if available {
            ArtifactBackupContent::Present
        } else if record.tombstone.is_some() {
            ArtifactBackupContent::RetiredMetadataOnly
        } else {
            return Err(ArtifactStoreError::InvalidBackup);
        };
        Ok(ArtifactBackupEntry { record, content })
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
            .await
            .map_err(backup_content_failure)?;
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
}
