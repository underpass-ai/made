use made_core::ports::{ArtifactRecord, ArtifactRetentionActor, ArtifactRetentionPolicy};
use made_core::value_objects::StepLease;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
/// A reviewed retention selection. Nothing is retired by constructing it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRetentionPlan {
    pub version: u32,
    pub observed_before: OffsetDateTime,
    pub retired_at: OffsetDateTime,
    pub actor: ArtifactRetentionActor,
    pub policy: ArtifactRetentionPolicy,
    pub lease: StepLease,
    pub records: Vec<ArtifactRecord>,
}
