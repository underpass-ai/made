use made_core::ports::ProtocolStepHandlerRequest;
use made_core::value_objects::{
    AuditActorKind, StateIteration, StateVisit, StepClaimFence, StepIteration,
};

/// One accepted claim and the semantic handler input it may execute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecuteProtocolOperationInput {
    pub handler_request: ProtocolStepHandlerRequest,
    pub state_visit: StateVisit,
    pub state_iteration: StateIteration,
    pub step_iteration: StepIteration,
    pub claim_fence: StepClaimFence,
    pub actor_kind: AuditActorKind,
}
