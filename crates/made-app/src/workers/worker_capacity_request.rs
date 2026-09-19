use made_core::value_objects::{
    CeremonyId, ExecutionConnectorId, ExecutionOperationId, LeaseOwnerId, StepId,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Durable admission identity, before an authoritative journal claim exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerCapacityRequest {
    pub operation_id: ExecutionOperationId,
    pub ceremony_id: CeremonyId,
    pub step_id: StepId,
    pub root_id: CeremonyId,
    pub connector_id: ExecutionConnectorId,
    pub provider_id: ExecutionConnectorId,
    pub owner_id: LeaseOwnerId,
    #[serde(with = "time::serde::rfc3339")]
    pub pending_until: OffsetDateTime,
}
