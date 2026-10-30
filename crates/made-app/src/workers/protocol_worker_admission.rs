use made_core::value_objects::{ExecutionOperationId, ProtocolId};

use super::{
    ProtocolWorkerCapacity, ProtocolWorkerCost, ProtocolWorkerEligibility, ProtocolWorkerPriority,
    ProtocolWorkerWeight,
};

/// One typed unit submitted to the scheduler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolWorkerScheduleRequest {
    root_id: ProtocolId,
    operation_id: ExecutionOperationId,
    priority: ProtocolWorkerPriority,
    weight: ProtocolWorkerWeight,
    cost: ProtocolWorkerCost,
    requested_capacity: ProtocolWorkerCapacity,
    eligibility: ProtocolWorkerEligibility,
}

impl ProtocolWorkerScheduleRequest {
    #[must_use]
    pub const fn new(
        root_id: ProtocolId,
        operation_id: ExecutionOperationId,
        priority: ProtocolWorkerPriority,
        weight: ProtocolWorkerWeight,
        cost: ProtocolWorkerCost,
        requested_capacity: ProtocolWorkerCapacity,
        eligibility: ProtocolWorkerEligibility,
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
    pub const fn root_id(&self) -> &ProtocolId {
        &self.root_id
    }
    #[must_use]
    pub const fn operation_id(&self) -> &ExecutionOperationId {
        &self.operation_id
    }
    #[must_use]
    pub const fn priority(&self) -> ProtocolWorkerPriority {
        self.priority
    }
    #[must_use]
    pub const fn weight(&self) -> ProtocolWorkerWeight {
        self.weight
    }
    #[must_use]
    pub const fn cost(&self) -> ProtocolWorkerCost {
        self.cost
    }
    #[must_use]
    pub const fn requested_capacity(&self) -> ProtocolWorkerCapacity {
        self.requested_capacity
    }
    #[must_use]
    pub const fn eligibility(&self) -> ProtocolWorkerEligibility {
        self.eligibility
    }
    #[must_use]
    pub const fn with_eligibility(mut self, eligibility: ProtocolWorkerEligibility) -> Self {
        self.eligibility = eligibility;
        self
    }
}
