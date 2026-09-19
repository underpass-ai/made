use made_core::value_objects::MaxParallel;

use super::{CeremonyWorkerCapacity, CeremonyWorkerPolicyVersion};

/// Bounded scheduler policy supplied by the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CeremonyWorkerSchedulerPolicy {
    max_parallel: MaxParallel,
    capacity: CeremonyWorkerCapacity,
    version: CeremonyWorkerPolicyVersion,
}

impl CeremonyWorkerSchedulerPolicy {
    #[must_use]
    pub const fn new(
        max_parallel: MaxParallel,
        capacity: CeremonyWorkerCapacity,
        version: CeremonyWorkerPolicyVersion,
    ) -> Self {
        Self {
            max_parallel,
            capacity,
            version,
        }
    }
    #[must_use]
    pub const fn max_parallel(self) -> MaxParallel {
        self.max_parallel
    }
    #[must_use]
    pub const fn capacity(self) -> CeremonyWorkerCapacity {
        self.capacity
    }
    #[must_use]
    pub const fn version(self) -> CeremonyWorkerPolicyVersion {
        self.version
    }
}
