use super::ArtifactRetentionPlan;
use made_core::ports::ArtifactTombstone;
use made_core::value_objects::ArtifactId;
use serde::{Deserialize, Serialize};
/// Audit result for either a dry-run or an explicit retention apply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRetentionReport {
    pub dry_run: bool,
    pub planned: Vec<ArtifactId>,
    pub applied: Vec<ArtifactTombstone>,
    pub already_retired: Vec<ArtifactId>,
}

impl ArtifactRetentionReport {
    pub(super) fn dry_run(plan: &ArtifactRetentionPlan) -> Self {
        Self {
            dry_run: true,
            planned: plan
                .records
                .iter()
                .map(|record| record.artifact.artifact_id().clone())
                .collect(),
            applied: Vec::new(),
            already_retired: Vec::new(),
        }
    }
}
