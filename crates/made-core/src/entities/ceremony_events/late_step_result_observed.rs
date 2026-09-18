use crate::value_objects::LateStepResult;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LateStepResultObserved {
    pub result: LateStepResult,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
}
