use made_core::value_objects::{CeremonyId, ExecutionOperationId};

use super::{
    CeremonyWorkerCapacity, CeremonyWorkerCost, CeremonyWorkerEligibility, CeremonyWorkerPriority,
    CeremonyWorkerWeight,
};

/// One typed unit submitted to the scheduler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyWorkerScheduleRequest {
    root_id: CeremonyId,
    operation_id: ExecutionOperationId,
    priority: CeremonyWorkerPriority,
    weight: CeremonyWorkerWeight,
    cost: CeremonyWorkerCost,
    requested_capacity: CeremonyWorkerCapacity,
    eligibility: CeremonyWorkerEligibility,
}

impl CeremonyWorkerScheduleRequest {
    #[must_use]
    pub const fn new(
        root_id: CeremonyId,
        operation_id: ExecutionOperationId,
        priority: CeremonyWorkerPriority,
        weight: CeremonyWorkerWeight,
        cost: CeremonyWorkerCost,
        requested_capacity: CeremonyWorkerCapacity,
        eligibility: CeremonyWorkerEligibility,
    ) -> Self {
        Self {
            root_id,
            operation_id,
            priority,
            weight,
            cost,
            requested_capacity,
            eligibility,
        }
    }
    #[must_use]
    pub const fn root_id(&self) -> &CeremonyId {
        &self.root_id
    }
    #[must_use]
    pub const fn operation_id(&self) -> &ExecutionOperationId {
        &self.operation_id
    }
    #[must_use]
    pub const fn priority(&self) -> CeremonyWorkerPriority {
        self.priority
    }
    #[must_use]
    pub const fn weight(&self) -> CeremonyWorkerWeight {
        self.weight
    }
    #[must_use]
    pub const fn cost(&self) -> CeremonyWorkerCost {
        self.cost
    }
    #[must_use]
    pub const fn requested_capacity(&self) -> CeremonyWorkerCapacity {
        self.requested_capacity
    }
    #[must_use]
    pub const fn eligibility(&self) -> CeremonyWorkerEligibility {
        self.eligibility
    }
    #[must_use]
    pub const fn with_eligibility(mut self, eligibility: CeremonyWorkerEligibility) -> Self {
        self.eligibility = eligibility;
        self
    }
}
