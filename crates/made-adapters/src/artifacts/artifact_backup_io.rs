use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use made_core::ports::{
    ArtifactByteOffset, ArtifactChunkLimit, ArtifactIdempotencyKey, ArtifactStoreError,
    ArtifactStorePort, ReadArtifactChunk,
};
use made_core::value_objects::ArtifactDigest;

use super::hashing::{digest_bytes, digest_reader, stable_key};
use super::local_artifact_io::write_json_atomic;
use super::ArtifactBackupEntry;

pub(super) async fn write_backup_blob<S: ArtifactStorePort>(
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
            .await
            .map_err(backup_content_failure)?;
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

pub(super) fn verify_backup_blob(
    root: &Path,
    entry: &ArtifactBackupEntry,
) -> Result<(), ArtifactStoreError> {
    let reference = &entry.record.artifact;
    verify_blob(
        &blob_path(root, entry.artifact_id().as_str()),
        reference.digest(),
        reference.size_bytes().get(),
    )
}

/// Keep backup integrity failures stable regardless of when a store detects them.
pub(super) fn backup_content_failure(error: ArtifactStoreError) -> ArtifactStoreError {
    match error {
        ArtifactStoreError::ChunkDigestMismatch
        | ArtifactStoreError::FinalDigestMismatch
        | ArtifactStoreError::Incomplete { .. } => ArtifactStoreError::InvalidBackup,
        other => other,
    }
}

pub(super) fn verify_blob(
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

pub(super) fn blob_path(root: &Path, artifact_id: &str) -> PathBuf {
    root.join("blobs").join(stable_key(artifact_id))
}

pub(super) fn staging_path(destination: &Path) -> PathBuf {
    destination.with_file_name(format!(
        ".{}-in-progress",
        destination
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("artifact-backup")
    ))
}

pub(super) fn read_json<T: serde::de::DeserializeOwned>(
    path: &Path,
) -> Result<T, ArtifactStoreError> {
    let bytes = fs::read(path).map_err(|_| ArtifactStoreError::InvalidBackup)?;
    serde_json::from_slice(&bytes).map_err(|_| ArtifactStoreError::InvalidBackup)
}

pub(super) fn write_json_durable(
    path: &Path,
    value: &impl serde::Serialize,
) -> Result<(), ArtifactStoreError> {
    write_json_atomic(path, value)?;
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(storage_failure)
}

pub(super) fn sync_directory(path: &Path) -> Result<(), ArtifactStoreError> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(storage_failure)
}

pub(super) fn storage_failure(error: std::io::Error) -> ArtifactStoreError {
    tracing::error!(%error, "artifact backup operation failed");
    ArtifactStoreError::unavailable("local artifact operation", error)
}

pub(super) fn backup_key(root: &Path) -> Result<ArtifactIdempotencyKey, ArtifactStoreError> {
    let path = fs::canonicalize(root).map_err(storage_failure)?;
    ArtifactIdempotencyKey::new(format!("backup:{}", stable_key(&path.to_string_lossy())))
        .map_err(Into::into)
}
