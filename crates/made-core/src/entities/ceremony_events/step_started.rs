use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::value_objects::{
    ContextKey, RoleId, StateIteration, StepAttempt, StepId, StepIteration, StepLease,
};

/// A seat took a step to run.
///
/// The lease carries the idempotency key the aggregate records, and the
/// attempt is the one the aggregate assigned — a fold applies it rather
/// than recomputing it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepStarted {
    pub step_id: StepId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_visit: Option<crate::value_objects::StateVisit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_iteration: Option<StateIteration>,
    pub iteration: StepIteration,
    pub attempt: StepAttempt,
    pub lease: StepLease,
    pub started_by: RoleId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_from: Option<ContextKey>,
    /// The actual role accepted for a static concurrent step when replay cannot
    /// recover it from the definition's canonical owner alone. This covers
    /// mixed dynamic/static states and explicitly selected alternate roles;
    /// canonical static-only claims retain the legacy unmarked shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sealed_role: Option<RoleId>,
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
}

impl StepStarted {
    #[must_use]
    pub fn state_iteration(&self) -> StateIteration {
        self.state_iteration.unwrap_or(StateIteration::FIRST)
    }

    #[must_use]
    pub fn state_visit(&self) -> crate::value_objects::StateVisit {
        self.state_visit.unwrap_or_default()
    }
}
