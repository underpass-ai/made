use made_core::value_objects::MaxParallel;

use super::{ProtocolWorkerCapacity, ProtocolWorkerPolicyVersion};

/// Bounded scheduler policy supplied by the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolWorkerSchedulerPolicy {
    max_parallel: MaxParallel,
    capacity: ProtocolWorkerCapacity,
    version: ProtocolWorkerPolicyVersion,
}

impl ProtocolWorkerSchedulerPolicy {
    #[must_use]
    pub const fn new(
        max_parallel: MaxParallel,
        capacity: ProtocolWorkerCapacity,
        version: ProtocolWorkerPolicyVersion,
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
    pub const fn capacity(self) -> ProtocolWorkerCapacity {
        self.capacity
    }
    #[must_use]
    pub const fn version(self) -> ProtocolWorkerPolicyVersion {
        self.version
    }
}
