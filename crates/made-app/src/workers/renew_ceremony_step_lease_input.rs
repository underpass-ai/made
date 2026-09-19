use made_core::value_objects::{
    CeremonyId, LeaseOwnerId, StepClaimFence, StepId, StepLeaseRenewalRequest,
};

/// One delegated renewal. Reuse the request identity only for the same payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenewCeremonyStepLeaseInput {
    pub ceremony_id: CeremonyId,
    pub step_id: StepId,
    pub claim_fence: StepClaimFence,
    pub owner: LeaseOwnerId,
    pub request: StepLeaseRenewalRequest,
}
