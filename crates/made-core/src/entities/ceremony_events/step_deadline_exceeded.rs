use crate::value_objects::{StepDeadline, StepResult};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepDeadlineExceeded {
    pub deadline: StepDeadline,
    pub result: StepResult,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
}
