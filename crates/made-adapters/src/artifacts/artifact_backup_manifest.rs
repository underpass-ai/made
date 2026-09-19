pub use super::artifact_backup_plan::ArtifactBackupPlan;
use serde::{Deserialize, Serialize};
pub const ARTIFACT_BACKUP_VERSION: u32 = 2;
/// Versioned progress metadata written after every durable backup blob.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactBackupManifest {
    pub version: u32,
    pub plan: ArtifactBackupPlan,
    pub completed: Vec<made_core::value_objects::ArtifactId>,
    pub complete: bool,
    #[serde(default)]
    pub protection_key: Option<made_core::ports::ArtifactIdempotencyKey>,
}

impl ArtifactBackupManifest {
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    pub(super) fn validate(&self) -> bool {
        self.version == ARTIFACT_BACKUP_VERSION
            && self.plan.validate()
            && self
                .completed
                .windows(2)
                .all(|window| window[0] < window[1])
            && self.completed.iter().all(|id| {
                self.plan
                    .entries
                    .iter()
                    .any(|entry| entry.artifact_id() == id)
            })
            && (!self.complete || self.completed.len() == self.plan.entries.len())
    }
}
