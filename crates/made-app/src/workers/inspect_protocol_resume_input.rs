use made_core::value_objects::{ExecutionRecoveryPageLimit, ProtocolId, StepClaimFence};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectProtocolResumeInput {
    pub protocol_id: ProtocolId,
    pub after_claim: Option<StepClaimFence>,
    pub limit: ExecutionRecoveryPageLimit,
}
