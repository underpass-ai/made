use std::fs::{File, OpenOptions};
use std::path::Path;
use std::sync::Arc;

use fs2::FileExt;
use made_core::ports::{ArtifactIdempotencyKey, ArtifactStoreError, ArtifactStorePort};

use super::hashing::digest_reader;
use super::local_artifact_io::{read_json, sync_directory, write_json_atomic};
use super::{
    ArtifactBackupService, LocalArtifactStore, SqliteBackupService, SqliteBackupSetManifest,
};

const DATABASE_DIR: &str = "database";
const ARTIFACT_DIR: &str = "artifacts";
const SET_MANIFEST: &str = "backup-set.json";

/// Composes one online SQLite snapshot with its exact external artifact blobs.
#[derive(Debug, Clone)]
pub struct SqliteArtifactBackupService {
    database: SqliteBackupService,
    artifacts: Arc<LocalArtifactStore>,
}

impl SqliteArtifactBackupService {
    #[must_use]
    pub fn new(artifacts: LocalArtifactStore, database: impl AsRef<Path>) -> Self {
        let artifacts = Arc::new(artifacts);
        Self {
            database: SqliteBackupService::new((*artifacts).clone(), database),
            artifacts,
        }
    }

    pub async fn backup_to(
        &self,
        destination: impl AsRef<Path>,
        key: ArtifactIdempotencyKey,
    ) -> Result<SqliteBackupSetManifest, ArtifactStoreError> {
        let root = destination.as_ref();
        std::fs::create_dir_all(root).map_err(storage_failure)?;
        if let Some(manifest) = read_json::<SqliteBackupSetManifest>(&root.join(SET_MANIFEST))? {
            if manifest.protection_key != key {
                return Err(ArtifactStoreError::IdempotencyConflict);
            }
            self.database
                .inspect_current(root.join(DATABASE_DIR), &key)
                .await?;
            self.verify(root, &manifest)?;
            self.database.release(root.join(DATABASE_DIR)).await?;
            return Ok(manifest);
        }

        let snapshot = self.database.prepare(root.join(DATABASE_DIR), key).await?;
        ArtifactBackupService::new(self.artifacts.clone())
            .backup_snapshot_to(root.join(ARTIFACT_DIR), &snapshot)
            .await?;

        let manifest = SqliteBackupSetManifest {
            version: SqliteBackupSetManifest::VERSION,
            protection_key: snapshot.key,
            database_manifest_digest: file_digest(
                &root.join(DATABASE_DIR).join("sqlite-manifest.json"),
            )?,
            artifact_manifest_digest: file_digest(&root.join(ARTIFACT_DIR).join("manifest.json"))?,
            verified_at: time::OffsetDateTime::now_utc(),
        };
        Self::verify_components(root)?;
        write_json_atomic(&root.join(SET_MANIFEST), &manifest)?;
        sync_directory(root)?;
        self.verify(root, &manifest)?;
        self.database.release(root.join(DATABASE_DIR)).await?;
        Ok(manifest)
    }

    pub fn verify(
        &self,
        source: impl AsRef<Path>,
        manifest: &SqliteBackupSetManifest,
    ) -> Result<(), ArtifactStoreError> {
        let root = source.as_ref();
        if !manifest.validate()
            || file_digest(&root.join(DATABASE_DIR).join("sqlite-manifest.json"))?
                != manifest.database_manifest_digest
            || file_digest(&root.join(ARTIFACT_DIR).join("manifest.json"))?
                != manifest.artifact_manifest_digest
        {
            return Err(ArtifactStoreError::InvalidBackup);
        }
        Self::verify_components(root)
    }

    fn verify_components(root: &Path) -> Result<(), ArtifactStoreError> {
        let database = SqliteBackupService::inspect(root.join(DATABASE_DIR))?;
        let artifacts =
            ArtifactBackupService::<LocalArtifactStore>::inspect_manifest(root.join(ARTIFACT_DIR))?;
        if artifacts.protection_key.as_ref() != Some(&database.protection_key)
            || artifacts
                .plan
                .entries
                .iter()
                .map(|entry| &entry.record)
                .ne(database.artifact_records.iter())
        {
            return Err(ArtifactStoreError::InvalidBackup);
        }
        Ok(())
    }

    /// Restore both components into a newly published directory. The target
    /// appears only after the database and every blob have verified.
    pub async fn restore_set_to(
        &self,
        source: impl AsRef<Path>,
        destination: impl AsRef<Path>,
    ) -> Result<(), ArtifactStoreError> {
        let source = source.as_ref();
        let destination = destination.as_ref();
        let parent = destination.parent().ok_or_else(|| {
            ArtifactStoreError::unavailable_static(
                "artifact record is missing its required metadata",
            )
        })?;
        std::fs::create_dir_all(parent).map_err(storage_failure)?;
        let name = destination
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                ArtifactStoreError::unavailable_static(
                    "artifact record is missing its required metadata",
                )
            })?;
        let lock_path = parent.join(format!(".{name}.restore-lock"));
        let _publication_lock = tokio::task::spawn_blocking(move || {
            let lock = OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(lock_path)
                .map_err(storage_failure)?;
            lock.lock_exclusive().map_err(storage_failure)?;
            Ok::<_, ArtifactStoreError>(lock)
        })
        .await
        .map_err(|error| {
            ArtifactStoreError::unavailable("serialize or decode artifact metadata", error)
        })??;
        if destination.exists() {
            return Err(ArtifactStoreError::IdempotencyConflict);
        }
        let set: SqliteBackupSetManifest =
            read_json(&source.join(SET_MANIFEST))?.ok_or(ArtifactStoreError::InvalidBackup)?;
        self.verify(source, &set)?;
        let staging = destination.with_extension(format!("restore-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&staging).map_err(storage_failure)?;
        SqliteBackupService::restore_to(
            source.join(DATABASE_DIR),
            staging.join("database.sqlite3"),
        )?;
        let target_store = Arc::new(LocalArtifactStore::open(staging.join("artifacts"))?);
        let artifact_restore = ArtifactBackupService::new(target_store.clone());
        artifact_restore
            .restore_from_protected(source.join(ARTIFACT_DIR))
            .await?;
        let database_manifest = SqliteBackupService::inspect(source.join(DATABASE_DIR))?;
        for protection in database_manifest.protections {
            target_store
                .protect_references(
                    protection.key,
                    protection
                        .records
                        .into_iter()
                        .map(|record| record.artifact.artifact_id().clone())
                        .collect(),
                )
                .await?;
        }
        artifact_restore
            .finish_restore(source.join(ARTIFACT_DIR))
            .await?;
        write_json_atomic(&staging.join("restore-complete.json"), &set)?;
        sync_directory(&staging)?;
        if destination.exists() {
            return Err(ArtifactStoreError::IdempotencyConflict);
        }
        std::fs::rename(&staging, destination).map_err(storage_failure)?;
        sync_directory(parent)
    }
}

fn file_digest(
    path: &Path,
) -> Result<made_core::value_objects::ArtifactDigest, ArtifactStoreError> {
    digest_reader(File::open(path).map_err(storage_failure)?)
        .map(|(digest, _)| digest)
        .map_err(storage_failure)
}

fn storage_failure(error: std::io::Error) -> ArtifactStoreError {
    tracing::error!(%error, "SQLite composed backup operation failed");
    ArtifactStoreError::unavailable("local artifact operation", error)
}
