use crate::services::LoadedSession;
use made_core::value_objects::{
    RoleId, StateIteration, StateVisit, StepAttempt, StepIteration, StepResult,
};

/// Result paired with the coordinates captured from its accepted claim.
pub(super) struct RunStepOutput {
    pub(super) session: LoadedSession,
    pub(super) role_id: RoleId,
    pub(super) state_visit: StateVisit,
    pub(super) state_iteration: StateIteration,
    pub(super) iteration: StepIteration,
    pub(super) attempt: StepAttempt,
    pub(super) result: StepResult,
}
