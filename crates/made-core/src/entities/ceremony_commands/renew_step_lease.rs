use crate::value_objects::{LeaseOwnerId, StepClaimFence, StepId};
use time::OffsetDateTime;

/// Extend the authority of an existing producer without creating another claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenewStepLease {
    pub request: Option<crate::value_objects::StepLeaseRenewalRequest>,
    pub step_id: StepId,
    pub claim_fence: StepClaimFence,
    pub lease_owner_id: LeaseOwnerId,
    pub expected_expires_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
    pub now: OffsetDateTime,
}
