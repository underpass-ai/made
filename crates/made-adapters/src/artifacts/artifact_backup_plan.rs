use super::artifact_backup_entry::ArtifactBackupEntry;
use super::artifact_backup_manifest::ARTIFACT_BACKUP_VERSION;
use super::hashing::digest_bytes;
use made_core::value_objects::ArtifactDigest;
use serde::{Deserialize, Serialize};
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
