use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::value_objects::{RoleId, StepAttempt, StepId, StepIteration, StepLease};

/// A seat took a step to run.
///
/// The lease carries the idempotency key the aggregate records, and the
/// attempt is the one the aggregate assigned — a fold applies it rather
/// than recomputing it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepStarted {
    pub step_id: StepId,
    pub iteration: StepIteration,
    pub attempt: StepAttempt,
    pub lease: StepLease,
    pub started_by: RoleId,
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
}
