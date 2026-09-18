use made_core::value_objects::{ExecutionRecoveryPageLimit, MaxParallel};

/// Bounded host policy; Corte 5 ola 1 deliberately keeps accepted leases fixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CeremonyWorkerPolicy {
    max_parallel: MaxParallel,
    recovery_page_limit: ExecutionRecoveryPageLimit,
}

impl CeremonyWorkerPolicy {
    #[must_use]
    pub const fn new(
        max_parallel: MaxParallel,
        recovery_page_limit: ExecutionRecoveryPageLimit,
    ) -> Self {
        Self {
            max_parallel,
            recovery_page_limit,
        }
    }

    #[must_use]
    pub const fn max_parallel(self) -> MaxParallel {
        self.max_parallel
    }

    #[must_use]
    pub const fn recovery_page_limit(self) -> ExecutionRecoveryPageLimit {
        self.recovery_page_limit
    }
}

impl Default for CeremonyWorkerPolicy {
    fn default() -> Self {
        Self::new(MaxParallel::DEFAULT, ExecutionRecoveryPageLimit::DEFAULT)
    }
}
