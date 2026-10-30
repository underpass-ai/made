use made_core::value_objects::{
    LeaseOwnerId, ProtocolId, StepClaimFence, StepId, StepLeaseRenewalRequest,
};

/// One delegated renewal. Reuse the request identity only for the same payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenewProtocolStepLeaseInput {
    pub protocol_id: ProtocolId,
    pub step_id: StepId,
    pub claim_fence: StepClaimFence,
    pub owner: LeaseOwnerId,
    pub request: StepLeaseRenewalRequest,
}
