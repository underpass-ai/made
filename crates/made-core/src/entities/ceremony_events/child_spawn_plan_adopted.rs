use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::value_objects::{ChildGroupId, StepClaimFence};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildSpawnPlanAdopted {
    pub group_id: ChildGroupId,
    pub claim_fence: StepClaimFence,
    #[serde(with = "time::serde::rfc3339")]
    pub adopted_at: OffsetDateTime,
}
