use time::OffsetDateTime;

use crate::value_objects::{MaxParallel, RoleId, StepId, StepLease};

/// Take a step to run under a lease.
///
/// `role_id` names the seat taking it and is checked against the
/// definition; absent, the engine takes the step and the event names
/// the seat the definition assigns to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartStep {
    pub role_id: Option<RoleId>,
    pub step_id: StepId,
    pub lease: StepLease,
    pub now: OffsetDateTime,
    pub max_parallel_ceiling: MaxParallel,
}
