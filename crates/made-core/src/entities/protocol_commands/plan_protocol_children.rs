use time::OffsetDateTime;

use crate::value_objects::ChildSpawnPlan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanProtocolChildren {
    pub plan: ChildSpawnPlan,
    pub now: OffsetDateTime,
}
