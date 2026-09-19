use std::path::Path;
use std::sync::Arc;

use made_core::ports::{ArtifactSnapshot, ArtifactStoreError, ArtifactStorePort};

use super::artifact_backup_manifest::{ArtifactBackupManifest, ArtifactBackupPlan};
use super::artifact_backup_use_case::ArtifactBackupUseCase;
use super::artifact_backup_verification::inspect_backup_manifest;
use super::artifact_restore_use_case::ArtifactRestoreUseCase;

/// Compatibility facade over the independent artifact backup and restore cases.
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

    pub async fn plan(&self) -> Result<ArtifactBackupPlan, ArtifactStoreError> {
        self.backup().plan().await
    }

    pub async fn prepare(
        &self,
        destination: impl AsRef<Path>,
    ) -> Result<ArtifactBackupPlan, ArtifactStoreError> {
        self.backup().prepare(destination).await
    }

    pub async fn prepare_snapshot(
        &self,
        destination: impl AsRef<Path>,
        snapshot: &ArtifactSnapshot,
    ) -> Result<ArtifactBackupPlan, ArtifactStoreError> {
        self.backup().prepare_snapshot(destination, snapshot).await
    }

    pub async fn backup_snapshot_to(
        &self,
        destination: impl AsRef<Path>,
        snapshot: &ArtifactSnapshot,
    ) -> Result<(), ArtifactStoreError> {
        self.backup()
            .backup_snapshot_to(destination, snapshot)
            .await
    }

    pub async fn backup_to(&self, destination: impl AsRef<Path>) -> Result<(), ArtifactStoreError> {
        self.backup().backup_to(destination).await
    }

    pub async fn abandon(&self, destination: impl AsRef<Path>) -> Result<(), ArtifactStoreError> {
        self.backup().abandon(destination).await
    }

    pub fn inspect_manifest(
        source: impl AsRef<Path>,
    ) -> Result<ArtifactBackupManifest, ArtifactStoreError> {
        inspect_backup_manifest(source.as_ref())
    }

    pub async fn restore_from(&self, source: impl AsRef<Path>) -> Result<(), ArtifactStoreError> {
        self.restore().restore_from(source).await
    }

    pub async fn restore_from_protected(
        &self,
        source: impl AsRef<Path>,
    ) -> Result<(), ArtifactStoreError> {
        self.restore().restore_from_protected(source).await
    }

    pub async fn finish_restore(&self, source: impl AsRef<Path>) -> Result<(), ArtifactStoreError> {
        self.restore().finish_restore(source).await
    }

    fn backup(&self) -> ArtifactBackupUseCase<S> {
        ArtifactBackupUseCase::new(self.store.clone())
    }

    fn restore(&self) -> ArtifactRestoreUseCase<S> {
        ArtifactRestoreUseCase::new(self.store.clone())
    }
}
