use made_core::ports::ArtifactRecord;
use serde::{Deserialize, Serialize};

/// One metadata record whose bytes are present in a verified backup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactBackupEntry {
    pub record: ArtifactRecord,
}

impl ArtifactBackupEntry {
    #[must_use]
    pub fn artifact_id(&self) -> &made_core::value_objects::ArtifactId {
        self.record.artifact.artifact_id()
    }
}
