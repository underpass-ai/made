use super::{
    ProtocolWorkerCapacity, ProtocolWorkerCost, ProtocolWorkerPriority, ProtocolWorkerWeight,
};

/// Trusted scheduling attributes selected for one protocol root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolWorkerRootPolicy {
    priority: ProtocolWorkerPriority,
    weight: ProtocolWorkerWeight,
    cost: ProtocolWorkerCost,
    requested_capacity: ProtocolWorkerCapacity,
}

impl ProtocolWorkerRootPolicy {
    #[must_use]
    pub const fn new(
        priority: ProtocolWorkerPriority,
        weight: ProtocolWorkerWeight,
        cost: ProtocolWorkerCost,
        requested_capacity: ProtocolWorkerCapacity,
    ) -> Self {
        Self {
            priority,
            weight,
            cost,
            requested_capacity,
        }
    }
    #[must_use]
    pub const fn priority(self) -> ProtocolWorkerPriority {
        self.priority
    }
    #[must_use]
    pub const fn weight(self) -> ProtocolWorkerWeight {
        self.weight
    }
    #[must_use]
    pub const fn cost(self) -> ProtocolWorkerCost {
        self.cost
    }
    #[must_use]
    pub const fn requested_capacity(self) -> ProtocolWorkerCapacity {
        self.requested_capacity
    }
}
