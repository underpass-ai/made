use super::{
    CeremonyWorkerCapacity, CeremonyWorkerCost, CeremonyWorkerPriority, CeremonyWorkerWeight,
};

/// Trusted scheduling attributes selected for one ceremony root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CeremonyWorkerRootPolicy {
    priority: CeremonyWorkerPriority,
    weight: CeremonyWorkerWeight,
    cost: CeremonyWorkerCost,
    requested_capacity: CeremonyWorkerCapacity,
}

impl CeremonyWorkerRootPolicy {
    #[must_use]
    pub const fn new(
        priority: CeremonyWorkerPriority,
        weight: CeremonyWorkerWeight,
        cost: CeremonyWorkerCost,
        requested_capacity: CeremonyWorkerCapacity,
    ) -> Self {
        Self {
            priority,
            weight,
            cost,
            requested_capacity,
        }
    }
    #[must_use]
    pub const fn priority(self) -> CeremonyWorkerPriority {
        self.priority
    }
    #[must_use]
    pub const fn weight(self) -> CeremonyWorkerWeight {
        self.weight
    }
    #[must_use]
    pub const fn cost(self) -> CeremonyWorkerCost {
        self.cost
    }
    #[must_use]
    pub const fn requested_capacity(self) -> CeremonyWorkerCapacity {
        self.requested_capacity
    }
}
