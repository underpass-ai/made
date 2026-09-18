use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::value_objects::{
    RoleId, StateIteration, StepAttempt, StepId, StepIteration, StepResult,
};

/// A step ended successfully, with its output.
///
/// Whether the step's repeat policy asked for another pass is decided
/// against the definition when the result is applied, so the outcome
/// of that decision travels here: `next_iteration` names the semantic
/// iteration the step was reopened at, and is absent when this result
/// is final for the step. A fold then reproduces the aggregate without
/// consulting the definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepCompleted {
    pub step_id: StepId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_visit: Option<crate::value_objects::StateVisit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_iteration: Option<StateIteration>,
    pub iteration: StepIteration,
    pub attempt: StepAttempt,
    pub result: StepResult,
    pub next_iteration: Option<StepIteration>,
    pub finished_by: RoleId,
    #[serde(with = "time::serde::rfc3339")]
    pub finished_at: OffsetDateTime,
}

impl StepCompleted {
    #[must_use]
    pub fn state_iteration(&self) -> StateIteration {
        self.state_iteration.unwrap_or(StateIteration::FIRST)
    }

    #[must_use]
    pub fn state_visit(&self) -> crate::value_objects::StateVisit {
        self.state_visit.unwrap_or_default()
    }
}
