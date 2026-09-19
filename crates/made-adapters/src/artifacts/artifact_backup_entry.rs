use made_core::ports::ArtifactRecord;
use serde::{Deserialize, Serialize};

/// One metadata record whose bytes are present in a verified backup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ArtifactBackupEntry {
    pub(super) record: ArtifactRecord,
}
