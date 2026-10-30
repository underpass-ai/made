use crate::value_objects::ProtocolDeadline;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolDeadlineExceeded {
    pub deadline: ProtocolDeadline,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
}
