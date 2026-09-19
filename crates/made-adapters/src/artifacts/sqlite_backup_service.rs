use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::Duration;

use made_core::ports::{
    ArtifactIdempotencyKey, ArtifactSnapshot, ArtifactStoreError, ArtifactStorePort,
};
use rusqlite::backup::Backup;
use rusqlite::{Connection, OpenFlags};

use super::hashing::{digest_bytes, digest_reader};
use super::local_artifact_io::{read_json, storage_failure, sync_directory, write_json_atomic};
use super::{LocalArtifactStore, SqliteBackupManifest};

const DATABASE_FILE: &str = "database.sqlite3";
const MANIFEST_FILE: &str = "sqlite-manifest.json";
const OWNER_FILE: &str = "sqlite-owner.json";

/// Online SQLite snapshot coordinated with a durable local-artifact pin.
#[derive(Debug, Clone)]
pub struct SqliteBackupService {
    artifacts: LocalArtifactStore,
    database: PathBuf,
}

impl SqliteBackupService {
    #[must_use]
    pub fn new(artifacts: LocalArtifactStore, database: impl AsRef<Path>) -> Self {
        Self {
            artifacts,
            database: database.as_ref().to_path_buf(),
        }
    }

    /// Capture or resume one database snapshot. The returned artifact snapshot
    /// is the exact selection callers must use for the companion blob backup.
    pub async fn prepare(
        &self,
        destination: impl AsRef<Path>,
        key: ArtifactIdempotencyKey,
    ) -> Result<ArtifactSnapshot, ArtifactStoreError> {
        let root = destination.as_ref();
        fs::create_dir_all(root).map_err(storage_failure)?;
        reject_live_destination(&self.database, &root.join(DATABASE_FILE))?;
        let identity = source_identity(&self.database)?;
        let artifact_store_identity = self.artifacts.store_identity().await?;

        if let Some(manifest) = read_json::<SqliteBackupManifest>(&root.join(MANIFEST_FILE))? {
            validate_manifest(root, &manifest)?;
            if manifest.protection_key != key
                || manifest.source_identity != identity
                || manifest.artifact_store_identity != artifact_store_identity
            {
                return Err(ArtifactStoreError::IdempotencyConflict);
            }
            let snapshot = self
                .artifacts
                .protect_references(
                    key,
                    manifest
                        .artifact_records
                        .iter()
                        .map(|record| record.artifact.artifact_id().clone())
                        .collect(),
                )
                .await?;
            if snapshot.records != manifest.artifact_records {
                return Err(ArtifactStoreError::IdempotencyConflict);
            }
            return Ok(snapshot);
        }

        let owner_path = root.join(OWNER_FILE);
        let existing_owner = match read_json::<(
            ArtifactIdempotencyKey,
            made_core::value_objects::ArtifactDigest,
            made_core::value_objects::ArtifactDigest,
        )>(&owner_path)?
        {
            Some(owner)
                if owner
                    != (
                        key.clone(),
                        identity.clone(),
                        artifact_store_identity.clone(),
                    ) =>
            {
                return Err(ArtifactStoreError::IdempotencyConflict)
            }
            Some(_) => true,
            None if root.join(DATABASE_FILE).exists() => {
                return Err(ArtifactStoreError::IdempotencyConflict)
            }
            None => {
                write_json_atomic(
                    &owner_path,
                    &(
                        key.clone(),
                        identity.clone(),
                        artifact_store_identity.clone(),
                    ),
                )?;
                false
            }
        };
        if existing_owner && !root.join(DATABASE_FILE).exists() {
            // The earlier process may have pinned an older artifact frontier.
            // Recapturing a later database under that selection could omit
            // newly referenced blobs, so recovery requires explicit abandon
            // and a fresh identity rather than guessing.
            return Err(ArtifactStoreError::IdempotencyConflict);
        }

        let source = self.database.clone();
        let snapshot_path = root.join(DATABASE_FILE);
        let (snapshot, protections) = self
            .artifacts
            .protect_snapshot_bundle_and_then(key.clone(), move || {
                capture_database(&source, &snapshot_path)
            })
            .await?;
        let (database_digest, database_bytes) =
            digest_reader(File::open(root.join(DATABASE_FILE)).map_err(storage_failure)?)
                .map_err(storage_failure)?;
        let artifact_records_digest = serde_json::to_vec(&snapshot.records)
            .map(|bytes| digest_bytes(&bytes))
            .map_err(|_| ArtifactStoreError::StorageUnavailable)?;
        let manifest = SqliteBackupManifest {
            version: SqliteBackupManifest::VERSION,
            protection_key: key,
            source_identity: identity,
            artifact_store_identity,
            database_digest,
            database_bytes,
            artifact_records_digest,
            artifact_records: snapshot.records.clone(),
            protections,
            captured_at: time::OffsetDateTime::now_utc(),
        };
        write_json_atomic(&root.join(MANIFEST_FILE), &manifest)?;
        sync_directory(root)?;
        Ok(snapshot)
    }

    pub fn inspect(source: impl AsRef<Path>) -> Result<SqliteBackupManifest, ArtifactStoreError> {
        let root = source.as_ref();
        let manifest = read_json::<SqliteBackupManifest>(&root.join(MANIFEST_FILE))?
            .ok_or(ArtifactStoreError::InvalidBackup)?;
        validate_manifest(root, &manifest)?;
        Ok(manifest)
    }

    pub async fn inspect_current(
        &self,
        source: impl AsRef<Path>,
        key: &ArtifactIdempotencyKey,
    ) -> Result<SqliteBackupManifest, ArtifactStoreError> {
        let manifest = Self::inspect(source)?;
        if &manifest.protection_key != key
            || manifest.source_identity != source_identity(&self.database)?
            || manifest.artifact_store_identity != self.artifacts.store_identity().await?
        {
            return Err(ArtifactStoreError::IdempotencyConflict);
        }
        Ok(manifest)
    }

    /// Restore a verified snapshot into a new path, never over the live source.
    pub fn restore_to(
        source: impl AsRef<Path>,
        destination: impl AsRef<Path>,
    ) -> Result<(), ArtifactStoreError> {
        let source = source.as_ref();
        let destination = destination.as_ref();
        Self::inspect(source)?;
        if destination.exists() {
            return Err(ArtifactStoreError::IdempotencyConflict);
        }
        reject_live_destination(&source.join(DATABASE_FILE), destination)?;
        copy_database(&source.join(DATABASE_FILE), destination)?;
        validate_sqlite(destination)
    }

    /// Release only after the companion blob backup is verified or abandonment
    /// has been explicitly authorized by the caller.
    pub async fn release(&self, source: impl AsRef<Path>) -> Result<(), ArtifactStoreError> {
        let manifest = Self::inspect(source)?;
        self.artifacts
            .release_snapshot(&manifest.protection_key)
            .await
    }
}

fn capture_database(source: &Path, destination: &Path) -> Result<(), ArtifactStoreError> {
    if destination.exists() {
        validate_sqlite(destination)?;
        return Ok(());
    }
    let parent = destination
        .parent()
        .ok_or(ArtifactStoreError::StorageUnavailable)?;
    fs::create_dir_all(parent).map_err(storage_failure)?;
    let temporary = destination.with_extension(format!("tmp-{}", uuid::Uuid::new_v4()));
    let source = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| sqlite_failure(&error))?;
    let mut target = Connection::open(&temporary).map_err(|error| sqlite_failure(&error))?;
    {
        let backup = Backup::new(&source, &mut target).map_err(|error| sqlite_failure(&error))?;
        backup
            .run_to_completion(128, Duration::from_millis(5), None)
            .map_err(|error| sqlite_failure(&error))?;
    }
    validate_connection(&target)?;
    drop(target);
    File::open(&temporary)
        .and_then(|file| file.sync_all())
        .map_err(storage_failure)?;
    fs::rename(&temporary, destination).map_err(storage_failure)?;
    sync_directory(parent)
}

fn copy_database(source: &Path, destination: &Path) -> Result<(), ArtifactStoreError> {
    let parent = destination
        .parent()
        .ok_or(ArtifactStoreError::StorageUnavailable)?;
    fs::create_dir_all(parent).map_err(storage_failure)?;
    let temporary = destination.with_extension(format!("tmp-{}", uuid::Uuid::new_v4()));
    fs::copy(source, &temporary).map_err(storage_failure)?;
    File::open(&temporary)
        .and_then(|file| file.sync_all())
        .map_err(storage_failure)?;
    fs::rename(&temporary, destination).map_err(storage_failure)?;
    sync_directory(parent)
}

fn validate_manifest(
    root: &Path,
    manifest: &SqliteBackupManifest,
) -> Result<(), ArtifactStoreError> {
    if !manifest.validate() {
        return Err(ArtifactStoreError::InvalidBackup);
    }
    let database = root.join(DATABASE_FILE);
    validate_sqlite(&database)?;
    let (digest, bytes) =
        digest_reader(File::open(database).map_err(storage_failure)?).map_err(storage_failure)?;
    if digest != manifest.database_digest || bytes != manifest.database_bytes {
        return Err(ArtifactStoreError::InvalidBackup);
    }
    Ok(())
}

fn validate_sqlite(path: &Path) -> Result<(), ArtifactStoreError> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| ArtifactStoreError::InvalidBackup)?;
    validate_connection(&connection)
}

fn validate_connection(connection: &Connection) -> Result<(), ArtifactStoreError> {
    let result: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|_| ArtifactStoreError::InvalidBackup)?;
    if result == "ok" {
        Ok(())
    } else {
        Err(ArtifactStoreError::InvalidBackup)
    }
}

fn source_identity(
    path: &Path,
) -> Result<made_core::value_objects::ArtifactDigest, ArtifactStoreError> {
    let canonical = fs::canonicalize(path).map_err(storage_failure)?;
    Ok(digest_bytes(canonical.to_string_lossy().as_bytes()))
}

fn reject_live_destination(source: &Path, destination: &Path) -> Result<(), ArtifactStoreError> {
    let source = fs::canonicalize(source).map_err(storage_failure)?;
    let destination = if destination.exists() {
        fs::canonicalize(destination).map_err(storage_failure)?
    } else {
        let parent = destination
            .parent()
            .ok_or(ArtifactStoreError::StorageUnavailable)?;
        fs::create_dir_all(parent).map_err(storage_failure)?;
        fs::canonicalize(parent).map_err(storage_failure)?.join(
            destination
                .file_name()
                .ok_or(ArtifactStoreError::StorageUnavailable)?,
        )
    };
    if source == destination {
        Err(ArtifactStoreError::IdempotencyConflict)
    } else {
        Ok(())
    }
}

fn sqlite_failure(error: &rusqlite::Error) -> ArtifactStoreError {
    tracing::error!(%error, "SQLite backup operation failed");
    ArtifactStoreError::StorageUnavailable
}
