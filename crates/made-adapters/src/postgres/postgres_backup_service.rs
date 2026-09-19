use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::Command;

use made_core::ports::{ArtifactIdempotencyKey, ArtifactStoreError, ArtifactStorePort};

use crate::artifacts::hashing::{digest_bytes, digest_reader};
use crate::artifacts::local_artifact_io::{read_json, sync_directory, write_json_atomic};

use super::{PostgresArtifactStore, PostgresBackupManifest, PostgresConfig, PostgresPool};

const ARCHIVE_FILE: &str = "database.dump";
const MANIFEST_FILE: &str = "postgres-manifest.json";
const OWNER_FILE: &str = "postgres-owner.json";

/// Full PostgreSQL backup. Artifact bytes share the database MVCC boundary.
#[derive(Clone)]
pub struct PostgresBackupService {
    artifacts: PostgresArtifactStore,
    database_url: String,
    pg_dump: PathBuf,
    pg_restore: PathBuf,
}

impl std::fmt::Debug for PostgresBackupService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PostgresBackupService")
            .field("database_url", &"[REDACTED]")
            .field("pg_dump", &self.pg_dump)
            .field("pg_restore", &self.pg_restore)
            .finish_non_exhaustive()
    }
}

impl PostgresBackupService {
    #[must_use]
    pub fn new(artifacts: PostgresArtifactStore, database_url: impl Into<String>) -> Self {
        Self {
            artifacts,
            database_url: database_url.into(),
            pg_dump: PathBuf::from("pg_dump"),
            pg_restore: PathBuf::from("pg_restore"),
        }
    }

    /// Override client binaries for a pinned operational toolchain.
    #[must_use]
    pub fn with_client_programs(
        mut self,
        pg_dump: impl Into<PathBuf>,
        pg_restore: impl Into<PathBuf>,
    ) -> Self {
        self.pg_dump = pg_dump.into();
        self.pg_restore = pg_restore.into();
        self
    }

    pub async fn backup_to(
        &self,
        destination: impl AsRef<Path>,
        key: ArtifactIdempotencyKey,
    ) -> Result<PostgresBackupManifest, ArtifactStoreError> {
        let root = destination.as_ref();
        fs::create_dir_all(root).map_err(storage_failure)?;
        let source_identity = digest_bytes(self.artifacts.database_identity().await?.as_bytes());
        if let Some(manifest) = read_json::<PostgresBackupManifest>(&root.join(MANIFEST_FILE))? {
            self.verify(root, &manifest)?;
            if manifest.protection_key != key || manifest.source_identity != source_identity {
                return Err(ArtifactStoreError::IdempotencyConflict);
            }
            self.artifacts.release_snapshot(&key).await?;
            return Ok(manifest);
        }

        let owner = (key.clone(), source_identity.clone());
        let owner_path = root.join(OWNER_FILE);
        if let Some(existing) = read_json::<(
            ArtifactIdempotencyKey,
            made_core::value_objects::ArtifactDigest,
        )>(&owner_path)?
        {
            if existing != owner {
                return Err(ArtifactStoreError::IdempotencyConflict);
            }
        } else {
            if root.join(ARCHIVE_FILE).exists() {
                return Err(ArtifactStoreError::IdempotencyConflict);
            }
            write_json_atomic(&owner_path, &owner)?;
        }

        self.artifacts.protect_snapshot(key.clone()).await?;
        let archive = root.join(ARCHIVE_FILE);
        if !archive.exists() {
            let temporary = archive.with_extension(format!("tmp-{}", uuid::Uuid::new_v4()));
            command_ok(
                Command::new(&self.pg_dump)
                    .arg("--format=custom")
                    .arg("--no-owner")
                    .arg("--no-privileges")
                    .arg("--file")
                    .arg(&temporary)
                    .arg(&self.database_url),
            )?;
            File::open(&temporary)
                .and_then(|file| file.sync_all())
                .map_err(storage_failure)?;
            fs::rename(temporary, &archive).map_err(storage_failure)?;
            sync_directory(root)?;
        }
        verify_archive(&self.pg_restore, &archive)?;
        let (archive_digest, archive_bytes) =
            digest_reader(File::open(&archive).map_err(storage_failure)?)
                .map_err(storage_failure)?;
        let manifest = PostgresBackupManifest {
            version: PostgresBackupManifest::VERSION,
            protection_key: key.clone(),
            source_identity,
            archive_digest,
            archive_bytes,
            captured_at: time::OffsetDateTime::now_utc(),
        };
        write_json_atomic(&root.join(MANIFEST_FILE), &manifest)?;
        sync_directory(root)?;
        self.verify(root, &manifest)?;
        self.artifacts.release_snapshot(&key).await?;
        Ok(manifest)
    }

    pub fn verify(
        &self,
        source: impl AsRef<Path>,
        manifest: &PostgresBackupManifest,
    ) -> Result<(), ArtifactStoreError> {
        if !manifest.validate() {
            return Err(ArtifactStoreError::InvalidBackup);
        }
        let archive = source.as_ref().join(ARCHIVE_FILE);
        verify_archive(&self.pg_restore, &archive)?;
        let (digest, bytes) = digest_reader(File::open(archive).map_err(storage_failure)?)
            .map_err(storage_failure)?;
        if digest != manifest.archive_digest || bytes != manifest.archive_bytes {
            return Err(ArtifactStoreError::InvalidBackup);
        }
        Ok(())
    }

    /// Restore into a separately provisioned database, never the source URL.
    pub async fn restore_to(
        &self,
        source: impl AsRef<Path>,
        isolated_database_url: &str,
    ) -> Result<(), ArtifactStoreError> {
        let manifest = read_json::<PostgresBackupManifest>(&source.as_ref().join(MANIFEST_FILE))?
            .ok_or(ArtifactStoreError::InvalidBackup)?;
        self.verify(source.as_ref(), &manifest)?;
        let target_pool = PostgresPool::connect(&PostgresConfig::from_url(isolated_database_url))
            .await
            .map_err(|error| {
                tracing::error!(%error, "PostgreSQL restore target connection failed");
                ArtifactStoreError::StorageUnavailable
            })?;
        let target = PostgresArtifactStore::new(target_pool.clone());
        if digest_bytes(target.database_identity().await?.as_bytes()) == manifest.source_identity {
            return Err(ArtifactStoreError::IdempotencyConflict);
        }
        let existing: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pg_catalog.pg_tables WHERE schemaname NOT IN ('pg_catalog', 'information_schema')",
        )
        .fetch_one(target_pool.inner())
        .await
        .map_err(|error| {
            tracing::error!(%error, "PostgreSQL restore target inspection failed");
            ArtifactStoreError::StorageUnavailable
        })?;
        if existing != 0 {
            return Err(ArtifactStoreError::IdempotencyConflict);
        }
        command_ok(
            Command::new(&self.pg_restore)
                .arg("--exit-on-error")
                .arg("--no-owner")
                .arg("--no-privileges")
                .arg("--dbname")
                .arg(isolated_database_url)
                .arg(source.as_ref().join(ARCHIVE_FILE)),
        )?;
        // The archive contains the backup protection as it existed at its own
        // MVCC boundary. It is no longer needed once restore succeeds.
        target.release_snapshot(&manifest.protection_key).await
    }
}

fn verify_archive(program: &Path, archive: &Path) -> Result<(), ArtifactStoreError> {
    command_ok(Command::new(program).arg("--list").arg(archive))
}

fn command_ok(command: &mut Command) -> Result<(), ArtifactStoreError> {
    let status = command.status().map_err(storage_failure)?;
    if status.success() {
        Ok(())
    } else {
        Err(ArtifactStoreError::InvalidBackup)
    }
}

fn storage_failure(error: std::io::Error) -> ArtifactStoreError {
    tracing::error!(%error, "PostgreSQL backup operation failed");
    drop(error);
    ArtifactStoreError::StorageUnavailable
}
