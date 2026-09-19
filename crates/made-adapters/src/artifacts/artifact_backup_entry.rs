use made_core::ports::ArtifactRecord;
use serde::{Deserialize, Serialize};

use super::ArtifactBackupContent;

/// One metadata record whose bytes are present in a verified backup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactBackupEntry {
    pub record: ArtifactRecord,
    #[serde(default)]
    pub content: ArtifactBackupContent,
}

impl ArtifactBackupEntry {
    #[must_use]
    pub fn artifact_id(&self) -> &made_core::value_objects::ArtifactId {
        self.record.artifact.artifact_id()
    }
}
