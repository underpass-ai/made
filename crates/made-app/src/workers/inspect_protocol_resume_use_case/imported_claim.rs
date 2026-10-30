use made_core::value_objects::{StepClaimFence, StepDeadline, StepExecutionRecord, StepId};

/// Immutable claim identity sealed in an `InstanceImported` snapshot.
#[derive(Clone)]
pub(super) struct ImportedClaim {
    pub(super) step_id: StepId,
    pub(super) record: StepExecutionRecord,
    pub(super) fence: StepClaimFence,
    pub(super) deadline: Option<StepDeadline>,
}
