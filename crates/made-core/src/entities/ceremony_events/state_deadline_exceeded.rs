use crate::value_objects::StateDeadline;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateDeadlineExceeded {
    pub deadline: StateDeadline,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
}
