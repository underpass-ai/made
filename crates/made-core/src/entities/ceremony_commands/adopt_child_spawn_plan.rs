use time::OffsetDateTime;

use crate::value_objects::{ChildGroupId, StepClaimFence};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdoptChildSpawnPlan {
    pub group_id: ChildGroupId,
    pub claim_fence: StepClaimFence,
    pub now: OffsetDateTime,
}
