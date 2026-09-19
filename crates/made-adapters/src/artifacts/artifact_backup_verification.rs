use std::path::Path;

use made_core::ports::ArtifactStoreError;

use super::artifact_backup_io::{read_json, verify_backup_blob};
use super::artifact_backup_manifest::ArtifactBackupManifest;
use super::ArtifactBackupContent;

pub(super) fn inspect_backup_manifest(
    source: &Path,
) -> Result<ArtifactBackupManifest, ArtifactStoreError> {
    let manifest: ArtifactBackupManifest = read_json(&source.join("manifest.json"))?;
    if !manifest.validate() || !manifest.complete {
        return Err(ArtifactStoreError::InvalidBackup);
    }
    for entry in &manifest.plan.entries {
        if entry.content == ArtifactBackupContent::Present {
            verify_backup_blob(source, entry)?;
        }
    }
    Ok(manifest)
}
