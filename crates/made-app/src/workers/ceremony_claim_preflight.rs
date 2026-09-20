use super::{CeremonyPreflightAction, ClaimExecutionEvidence};
use made_core::entities::ceremony_events::HostHandoffRecorded;
use made_core::value_objects::CeremonyClaimPhase;
use made_core::value_objects::{
    BudgetReservationId, ExecutionOperationId, LeaseOwnerId, StepClaimFence, StepId,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyClaimPreflight {
    pub step_id: StepId,
    pub claim_fence: StepClaimFence,
    pub owner: LeaseOwnerId,
    pub operation_id: ExecutionOperationId,
    pub phase: CeremonyClaimPhase,
    #[serde(with = "time::serde::rfc3339")]
    pub effective_lease_expires_at: OffsetDateTime,
    pub remaining_lease_ms: u64,
    #[serde(with = "time::serde::rfc3339::option")]
    pub step_deadline_at: Option<OffsetDateTime>,
    pub deadline_overdue: bool,
    pub budget_reservation_id: Option<BudgetReservationId>,
    pub host_declaration: Option<HostHandoffRecorded>,
    pub execution: ClaimExecutionEvidence,
    pub permitted_recovery_paths: Vec<CeremonyPreflightAction>,
}
