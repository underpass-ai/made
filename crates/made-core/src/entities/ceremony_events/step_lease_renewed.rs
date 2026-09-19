use crate::value_objects::{LeaseOwnerId, StepClaimFence, StepId};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// A durable extension; the original lease remains the accepted producer identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepLeaseRenewed {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<crate::value_objects::StepLeaseRenewalRequest>,
    pub step_id: StepId,
    pub claim_fence: StepClaimFence,
    pub lease_owner_id: LeaseOwnerId,
    #[serde(with = "time::serde::rfc3339")]
    pub previous_expires_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub renewed_at: OffsetDateTime,
}
