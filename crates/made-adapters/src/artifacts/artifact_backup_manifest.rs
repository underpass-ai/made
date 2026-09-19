use serde::{Deserialize, Serialize};

use super::artifact_backup_entry::ArtifactBackupEntry;

/// Versioned metadata written last after every backup blob is durable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ArtifactBackupManifest {
    pub(super) version: u32,
    pub(super) entries: Vec<ArtifactBackupEntry>,
}
