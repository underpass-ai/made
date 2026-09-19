use super::ArtifactGcPlan;
use made_core::value_objects::ArtifactDigest;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactGcReport {
    pub dry_run: bool,
    pub planned: Vec<ArtifactDigest>,
    pub deleted: Vec<ArtifactDigest>,
    #[serde(default)]
    pub reclaimed_bytes: u64,
}

impl ArtifactGcReport {
    /// Produce an auditable preview without deleting bytes.
    #[must_use]
    pub fn dry_run(plan: &ArtifactGcPlan) -> Self {
        Self {
            dry_run: true,
            planned: plan
                .candidates
                .iter()
                .map(|candidate| candidate.digest.clone())
                .collect(),
            deleted: Vec::new(),
            reclaimed_bytes: 0,
        }
    }
}
