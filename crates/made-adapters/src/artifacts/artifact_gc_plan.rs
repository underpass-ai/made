use super::{ArtifactGcCandidate, ArtifactGcExclusion};
use made_core::value_objects::StepLease;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
/// Reviewed, dry-run GC selection. Applying it is explicit and fenced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactGcPlan {
    pub version: u32,
    pub retire_before: OffsetDateTime,
    pub lease: StepLease,
    pub candidates: Vec<ArtifactGcCandidate>,
    #[serde(default)]
    pub exclusions: Vec<ArtifactGcExclusion>,
}
