use super::ProtocolWorkerRootPolicy;
use super::{
    ProtocolWorkerCapacity, ProtocolWorkerCost, ProtocolWorkerPolicyVersion,
    ProtocolWorkerPriority, ProtocolWorkerSchedulerPolicy, ProtocolWorkerWeight,
};

/// Versioned host policy applied uniformly to discovered worker candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolWorkerAdmissionPolicy {
    scheduler: ProtocolWorkerSchedulerPolicy,
    priority: ProtocolWorkerPriority,
    weight: ProtocolWorkerWeight,
    cost: ProtocolWorkerCost,
    requested_capacity: ProtocolWorkerCapacity,
}

impl ProtocolWorkerAdmissionPolicy {
    #[must_use]
    pub const fn new(
        scheduler: ProtocolWorkerSchedulerPolicy,
        priority: ProtocolWorkerPriority,
        weight: ProtocolWorkerWeight,
        cost: ProtocolWorkerCost,
        requested_capacity: ProtocolWorkerCapacity,
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
    pub const fn scheduler(self) -> ProtocolWorkerSchedulerPolicy {
        self.scheduler
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

    #[must_use]
    pub const fn root_policy(self) -> ProtocolWorkerRootPolicy {
        ProtocolWorkerRootPolicy::new(
            self.priority,
            self.weight,
            self.cost,
            self.requested_capacity,
        )
    }
}

impl Default for ProtocolWorkerAdmissionPolicy {
    fn default() -> Self {
        Self::new(
            ProtocolWorkerSchedulerPolicy::new(
                made_core::value_objects::MaxParallel::DEFAULT,
                ProtocolWorkerCapacity::new(1).expect("one is valid capacity"),
                ProtocolWorkerPolicyVersion::new(1).expect("one is valid version"),
            ),
            ProtocolWorkerPriority::DEFAULT,
            ProtocolWorkerWeight::new(1).expect("one is valid weight"),
            ProtocolWorkerCost::new(1).expect("one is valid cost"),
            ProtocolWorkerCapacity::new(1).expect("one is valid capacity"),
        )
    }
}
