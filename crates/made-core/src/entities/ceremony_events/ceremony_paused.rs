use crate::value_objects::LifecycleReason;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyPaused {
    pub reason: LifecycleReason,
    #[serde(with = "time::serde::rfc3339")]
    pub paused_at: OffsetDateTime,
}
