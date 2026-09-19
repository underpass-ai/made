use made_core::value_objects::ArtifactDigest;
use serde::{Deserialize, Serialize};

use super::artifact_backup_entry::ArtifactBackupEntry;
use super::hashing::digest_bytes;

pub const ARTIFACT_BACKUP_VERSION: u32 = 2;

/// The immutable, deterministic set of records selected for one backup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactBackupPlan {
    pub version: u32,
    pub entries: Vec<ArtifactBackupEntry>,
    pub plan_digest: ArtifactDigest,
}

impl ArtifactBackupPlan {
    pub(super) fn from_entries(
        mut entries: Vec<ArtifactBackupEntry>,
    ) -> Result<Self, serde_json::Error> {
        entries.sort_by(|left, right| left.artifact_id().cmp(right.artifact_id()));
        let bytes = serde_json::to_vec(&entries)?;
        Ok(Self {
            version: ARTIFACT_BACKUP_VERSION,
            entries,
            plan_digest: digest_bytes(&bytes),
        })
    }

    pub(super) fn validate(&self) -> bool {
        if self.version != ARTIFACT_BACKUP_VERSION
            || self
                .entries
                .windows(2)
                .any(|window| window[0].artifact_id() >= window[1].artifact_id())
        {
            return false;
        }
        serde_json::to_vec(&self.entries)
            .is_ok_and(|bytes| digest_bytes(&bytes) == self.plan_digest)
    }
}

/// Versioned progress metadata written after every durable backup blob.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactBackupManifest {
    pub version: u32,
    pub plan: ArtifactBackupPlan,
    pub completed: Vec<made_core::value_objects::ArtifactId>,
    pub complete: bool,
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
