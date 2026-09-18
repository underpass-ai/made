use time::OffsetDateTime;

use crate::value_objects::ChildSpawnPlan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanCeremonyChildren {
    pub plan: ChildSpawnPlan,
    pub now: OffsetDateTime,
}
