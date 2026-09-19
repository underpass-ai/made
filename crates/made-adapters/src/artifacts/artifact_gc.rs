use made_core::value_objects::{ArtifactDigest, ArtifactId, StepLease};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// One content-addressed blob safe to remove after every reference retired.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactGcCandidate {
    pub digest: ArtifactDigest,
    pub bytes: u64,
    pub artifact_ids: Vec<ArtifactId>,
}

/// Reviewed, dry-run GC selection. Applying it is explicit and fenced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactGcPlan {
    pub version: u32,
    pub retire_before: OffsetDateTime,
    pub lease: StepLease,
    pub candidates: Vec<ArtifactGcCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactGcReport {
    pub dry_run: bool,
    pub planned: Vec<ArtifactDigest>,
    pub deleted: Vec<ArtifactDigest>,
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
        }
    }
}
