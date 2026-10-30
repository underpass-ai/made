use crate::value_objects::LifecycleReason;
use time::OffsetDateTime;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PauseProtocol {
    pub reason: LifecycleReason,
    pub now: OffsetDateTime,
}
