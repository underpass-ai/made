use super::{ClaimExecutionEvidence, ProtocolPreflightAction};
use made_core::entities::protocol_events::HostHandoffRecorded;
use made_core::value_objects::ProtocolClaimPhase;
use made_core::value_objects::{
    BudgetReservationId, ExecutionOperationId, LeaseOwnerId, StepClaimFence, StepId,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolClaimPreflight {
    pub step_id: StepId,
    pub claim_fence: StepClaimFence,
    pub owner: LeaseOwnerId,
    pub operation_id: ExecutionOperationId,
    pub phase: ProtocolClaimPhase,
    #[serde(with = "time::serde::rfc3339")]
    pub effective_lease_expires_at: OffsetDateTime,
    pub remaining_lease_ms: u64,
    #[serde(with = "time::serde::rfc3339::option")]
    pub step_deadline_at: Option<OffsetDateTime>,
    pub deadline_overdue: bool,
    pub budget_reservation_id: Option<BudgetReservationId>,
    pub host_declaration: Option<HostHandoffRecorded>,
    pub execution: ClaimExecutionEvidence,
    pub permitted_recovery_paths: Vec<ProtocolPreflightAction>,
}
