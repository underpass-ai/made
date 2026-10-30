use time::OffsetDateTime;

use crate::value_objects::{RoleId, TransitionTrigger};

/// Move the session along a transition.
///
/// `role_id` is absent when the engine takes the move itself; the
/// record then names nobody rather than inventing someone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyTransition {
    pub role_id: Option<RoleId>,
    pub trigger: TransitionTrigger,
    pub now: OffsetDateTime,
}
