use made_core::ports::CeremonyStepHandlerRequest;
use made_core::value_objects::{
    RoleId, StateIteration, StateVisit, StepAttempt, StepClaimFence, StepId, StepIteration,
};

/// A durable claim and the exact execution identity its result must retain.
pub(super) struct ClaimedStep {
    pub(super) request: CeremonyStepHandlerRequest,
    pub(super) step_id: StepId,
    pub(super) role_id: RoleId,
    pub(super) claim_fence: StepClaimFence,
    pub(super) state_visit: StateVisit,
    pub(super) state_iteration: StateIteration,
    pub(super) iteration: StepIteration,
    pub(super) attempt: StepAttempt,
}
