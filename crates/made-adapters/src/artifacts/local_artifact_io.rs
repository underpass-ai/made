use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use made_core::ports::ArtifactStoreError;
use uuid::Uuid;

pub(crate) fn read_json<T: serde::de::DeserializeOwned>(
    path: &Path,
) -> Result<Option<T>, ArtifactStoreError> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map(Some).map_err(|error| {
            ArtifactStoreError::unavailable("serialize or decode artifact metadata", error)
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(storage_failure(error)),
    }
}

pub(crate) fn write_json_atomic(
    path: &Path,
    value: &impl serde::Serialize,
) -> Result<(), ArtifactStoreError> {
    let bytes = serde_json::to_vec(value).map_err(|error| {
        ArtifactStoreError::unavailable("serialize or decode artifact metadata", error)
    })?;
    let temporary = path.with_extension(format!("tmp-{}", Uuid::new_v4()));
    let mut file = File::create(&temporary).map_err(storage_failure)?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(storage_failure)?;
    fs::rename(&temporary, path).map_err(storage_failure)?;
    sync_directory(path.parent().expect("artifact state has a parent"))
}

pub(crate) fn sync_directory(path: &Path) -> Result<(), ArtifactStoreError> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(storage_failure)
}

pub(super) fn storage_failure(error: std::io::Error) -> ArtifactStoreError {
    tracing::error!(%error, "local artifact store operation failed");
    ArtifactStoreError::unavailable("local artifact operation", error)
}
