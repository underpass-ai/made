use crate::value_objects::CeremonyDeadline;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyDeadlineExceeded {
    pub deadline: CeremonyDeadline,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
}
