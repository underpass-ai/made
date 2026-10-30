use made_core::value_objects::{
    ProtocolId, RoleId, StateIteration, StateVisit, StepAttempt, StepClaimFence, StepId,
    StepIteration, StepResult,
};

/// A handler outcome still bound to the durable claim it must complete.
pub(super) struct ExecutedStep {
    pub(super) instance_id: ProtocolId,
    pub(super) step_id: StepId,
    pub(super) role_id: RoleId,
    pub(super) claim_fence: StepClaimFence,
    pub(super) state_visit: StateVisit,
    pub(super) state_iteration: StateIteration,
    pub(super) iteration: StepIteration,
    pub(super) attempt: StepAttempt,
    pub(super) result: StepResult,
}
