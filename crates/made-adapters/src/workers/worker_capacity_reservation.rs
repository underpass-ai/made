use made_app::workers::WorkerCapacityRequest;
use made_core::value_objects::StepClaimFence;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct WorkerCapacityReservation {
    pub request: WorkerCapacityRequest,
    pub fence: Option<StepClaimFence>,
}
