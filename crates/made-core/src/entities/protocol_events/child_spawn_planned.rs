use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::value_objects::ChildSpawnPlan;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildSpawnPlanned {
    pub plan: ChildSpawnPlan,
    #[serde(with = "time::serde::rfc3339")]
    pub planned_at: OffsetDateTime,
}
