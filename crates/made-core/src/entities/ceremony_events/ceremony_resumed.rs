use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyResumed {
    #[serde(with = "time::serde::rfc3339")]
    pub resumed_at: OffsetDateTime,
}
