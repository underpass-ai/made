use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::Command;

use made_core::ports::{
    ArtifactIdempotencyKey, ArtifactRecord, ArtifactStoreError, ArtifactStorePort,
};
use serde::{Deserialize, Serialize};

use crate::artifacts::hashing::{digest_bytes, digest_reader};
use crate::artifacts::local_artifact_io::{read_json, sync_directory, write_json_atomic};

use super::{PostgresArtifactStore, PostgresBackupManifest, PostgresConfig, PostgresPool};
use sqlx::{Postgres, Row, Transaction};

const ARCHIVE_FILE: &str = "database.dump";
const MANIFEST_FILE: &str = "postgres-manifest.json";
const OWNER_FILE: &str = "postgres-owner.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PostgresBackupOwner {
    protection_key: ArtifactIdempotencyKey,
    source_identity: made_core::value_objects::ArtifactDigest,
    snapshot_id: String,
    transaction_snapshot: String,
    artifact_records: Vec<ArtifactRecord>,
}

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

        let owner_path = root.join(OWNER_FILE);
        let existing_owner = read_json::<PostgresBackupOwner>(&owner_path)?;
        if let Some(owner) = &existing_owner {
            if owner.protection_key != key || owner.source_identity != source_identity {
                return Err(ArtifactStoreError::IdempotencyConflict);
            }
        } else if root.join(ARCHIVE_FILE).exists() {
            return Err(ArtifactStoreError::IdempotencyConflict);
        }

        let archive = root.join(ARCHIVE_FILE);
        let mut transaction = None;
        let owner = if archive.exists() {
            let owner = existing_owner.ok_or(ArtifactStoreError::IdempotencyConflict)?;
            let protected = self.artifacts.protect_snapshot(key.clone()).await?;
            if protected.records != owner.artifact_records {
                return Err(ArtifactStoreError::IdempotencyConflict);
            }
            owner
        } else {
            let protected = self.artifacts.protect_snapshot(key.clone()).await?;
            let (captured, snapshot_id, transaction_snapshot, artifact_records) =
                self.capture_snapshot().await?;
            if protected.records != artifact_records
                || existing_owner
                    .as_ref()
                    .is_some_and(|owner| owner.artifact_records != artifact_records)
            {
                // A concurrent artifact mutation occurred between the durable
                // pin and the exported MVCC snapshot. Keep the pin and fail
                // closed; retrying can safely recapture the same destination.
                return Err(ArtifactStoreError::IdempotencyConflict);
            }
            let owner = PostgresBackupOwner {
                protection_key: key.clone(),
                source_identity: source_identity.clone(),
                snapshot_id,
                transaction_snapshot,
                artifact_records,
            };
            write_json_atomic(&owner_path, &owner)?;
            transaction = Some(captured);
            owner
        };
        if !archive.exists() {
            let temporary = archive.with_extension(format!("tmp-{}", uuid::Uuid::new_v4()));
            command_ok(
                Command::new(&self.pg_dump)
                    .arg("--format=custom")
                    .arg("--no-owner")
                    .arg("--no-privileges")
                    .arg("--snapshot")
                    .arg(&owner.snapshot_id)
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
            snapshot_id: owner.snapshot_id,
            transaction_snapshot: owner.transaction_snapshot,
            artifact_records: owner.artifact_records,
            archive_digest,
            archive_bytes,
            captured_at: time::OffsetDateTime::now_utc(),
        };
        write_json_atomic(&root.join(MANIFEST_FILE), &manifest)?;
        sync_directory(root)?;
        self.verify(root, &manifest)?;
        if let Some(transaction) = transaction {
            transaction
                .commit()
                .await
                .map_err(|error| postgres_failure(&error))?;
        }
        self.artifacts.release_snapshot(&key).await?;
        Ok(manifest)
    }

    async fn capture_snapshot(
        &self,
    ) -> Result<
        (
            Transaction<'_, Postgres>,
            String,
            String,
            Vec<ArtifactRecord>,
        ),
        ArtifactStoreError,
    > {
        let mut transaction = self
            .artifacts
            .pool()
            .inner()
            .begin()
            .await
            .map_err(|error| postgres_failure(&error))?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .execute(&mut *transaction)
            .await
            .map_err(|error| postgres_failure(&error))?;
        let snapshot_id: String = sqlx::query_scalar("SELECT pg_export_snapshot()")
            .fetch_one(&mut *transaction)
            .await
            .map_err(|error| postgres_failure(&error))?;
        let transaction_snapshot: String =
            sqlx::query_scalar("SELECT txid_current_snapshot()::text")
                .fetch_one(&mut *transaction)
                .await
                .map_err(|error| postgres_failure(&error))?;
        let records = sqlx::query("SELECT body FROM artifact_records ORDER BY artifact_id")
            .fetch_all(&mut *transaction)
            .await
            .map_err(|error| postgres_failure(&error))?
            .into_iter()
            .map(|row| {
                serde_json::from_value(
                    row.try_get("body")
                        .map_err(|error| postgres_failure(&error))?,
                )
                .map_err(|_| ArtifactStoreError::StorageUnavailable)
            })
            .collect::<Result<Vec<ArtifactRecord>, _>>()?;
        Ok((transaction, snapshot_id, transaction_snapshot, records))
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

fn postgres_failure(error: &sqlx::Error) -> ArtifactStoreError {
    tracing::error!(%error, "PostgreSQL backup operation failed");
    ArtifactStoreError::StorageUnavailable
}
