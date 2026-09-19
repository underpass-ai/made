use super::CeremonyWorkerRootPolicy;
use super::{
    CeremonyWorkerCapacity, CeremonyWorkerCost, CeremonyWorkerPolicyVersion,
    CeremonyWorkerPriority, CeremonyWorkerSchedulerPolicy, CeremonyWorkerWeight,
};

/// Versioned host policy applied uniformly to discovered worker candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CeremonyWorkerAdmissionPolicy {
    scheduler: CeremonyWorkerSchedulerPolicy,
    priority: CeremonyWorkerPriority,
    weight: CeremonyWorkerWeight,
    cost: CeremonyWorkerCost,
    requested_capacity: CeremonyWorkerCapacity,
}

impl CeremonyWorkerAdmissionPolicy {
    #[must_use]
    pub const fn new(
        scheduler: CeremonyWorkerSchedulerPolicy,
        priority: CeremonyWorkerPriority,
        weight: CeremonyWorkerWeight,
        cost: CeremonyWorkerCost,
        requested_capacity: CeremonyWorkerCapacity,
    ) -> Self {
        Self {
            scheduler,
            priority,
            weight,
            cost,
            requested_capacity,
        }
    }

    #[must_use]
    pub const fn scheduler(self) -> CeremonyWorkerSchedulerPolicy {
        self.scheduler
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

    #[must_use]
    pub const fn root_policy(self) -> CeremonyWorkerRootPolicy {
        CeremonyWorkerRootPolicy::new(
            self.priority,
            self.weight,
            self.cost,
            self.requested_capacity,
        )
    }
}

impl Default for CeremonyWorkerAdmissionPolicy {
    fn default() -> Self {
        Self::new(
            CeremonyWorkerSchedulerPolicy::new(
                made_core::value_objects::MaxParallel::DEFAULT,
                CeremonyWorkerCapacity::new(1).expect("one is valid capacity"),
                CeremonyWorkerPolicyVersion::new(1).expect("one is valid version"),
            ),
            CeremonyWorkerPriority::DEFAULT,
            CeremonyWorkerWeight::new(1).expect("one is valid weight"),
            CeremonyWorkerCost::new(1).expect("one is valid cost"),
            CeremonyWorkerCapacity::new(1).expect("one is valid capacity"),
        )
    }
}
