use std::fs;
use std::path::{Path, PathBuf};

use made_core::ports::{ArtifactStoreError, ArtifactUploadId};
use made_core::value_objects::{ArtifactId, ArtifactRef};

use super::hashing::stable_key;
use super::local_artifact_io::storage_failure;

/// Filesystem layout for one local artifact store.
#[derive(Debug, Clone)]
pub(super) struct LocalArtifactLayout {
    root: PathBuf,
}

impl LocalArtifactLayout {
    pub(super) fn open(root: impl AsRef<Path>) -> Result<Self, ArtifactStoreError> {
        let layout = Self {
            root: root.as_ref().to_path_buf(),
        };
        for child in [
            "uploads",
            "idempotency",
            "artifacts",
            "blobs",
            "protections",
        ] {
            fs::create_dir_all(layout.root.join(child)).map_err(storage_failure)?;
        }
        Ok(layout)
    }

    pub(super) fn lock_path(&self) -> PathBuf {
        self.root.join(".artifact-store.lock")
    }

    pub(super) fn protections_dir(&self) -> PathBuf {
        self.root.join("protections")
    }

    pub(super) fn protection(&self, key: &str) -> PathBuf {
        self.protections_dir()
            .join(format!("{}.json", stable_key(key)))
    }

    pub(super) fn uploads_dir(&self) -> PathBuf {
        self.root.join("uploads")
    }

    pub(super) fn artifacts_dir(&self) -> PathBuf {
        self.root.join("artifacts")
    }

    pub(super) fn blobs_dir(&self) -> PathBuf {
        self.root.join("blobs")
    }

    pub(super) fn upload_manifest(&self, id: &ArtifactUploadId) -> PathBuf {
        self.uploads_dir()
            .join(format!("{}.json", stable_key(id.as_str())))
    }

    pub(super) fn upload_part(&self, id: &ArtifactUploadId) -> PathBuf {
        self.uploads_dir()
            .join(format!("{}.part", stable_key(id.as_str())))
    }

    pub(super) fn idempotency(&self, key: &str) -> PathBuf {
        self.root
            .join("idempotency")
            .join(format!("{}.json", stable_key(key)))
    }

    pub(super) fn artifact(&self, id: &ArtifactId) -> PathBuf {
        self.artifacts_dir()
            .join(format!("{}.json", stable_key(id.as_str())))
    }

    pub(super) fn blob(&self, artifact: &ArtifactRef) -> PathBuf {
        self.root
            .join("blobs")
            .join(artifact.digest().as_str().trim_start_matches("sha256:"))
    }
}
