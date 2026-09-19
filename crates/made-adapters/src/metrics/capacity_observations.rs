use std::sync::atomic::{AtomicU64, Ordering};

/// Bounded counters for the operational dimensions that C6 adds. There are
/// deliberately no caller-provided labels, so a ceremony/root/provider cannot
/// create unbounded metric cardinality.
#[derive(Debug, Default)]
pub struct CapacityObservations {
    admission_wait_millis: AtomicU64,
    admission_wait_samples: AtomicU64,
    admission_rejected: AtomicU64,
    active_capacity: AtomicU64,
    backpressure: AtomicU64,
    recovery: AtomicU64,
    reconciliation: AtomicU64,
    provider_fallback: AtomicU64,
    unknown_usage: AtomicU64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapacitySnapshot {
    pub admission_wait_millis: u64,
    pub admission_wait_samples: u64,
    pub admission_rejected: u64,
    pub active_capacity: u64,
    pub backpressure: u64,
    pub recovery: u64,
    pub reconciliation: u64,
    pub provider_fallback: u64,
    pub unknown_usage: u64,
}

impl CapacityObservations {
    pub fn record_admission_wait(&self, millis: u64) {
        self.admission_wait_millis
            .fetch_add(millis, Ordering::Relaxed);
        self.admission_wait_samples.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_admission_rejected(&self) {
        self.admission_rejected.fetch_add(1, Ordering::Relaxed);
    }

    pub fn set_active_capacity(&self, active: u64) {
        self.active_capacity.store(active, Ordering::Relaxed);
    }

    pub fn record_backpressure(&self) {
        self.backpressure.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_recovery(&self) {
        self.recovery.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_reconciliation(&self) {
        self.reconciliation.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_provider_fallback(&self) {
        self.provider_fallback.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_unknown_usage(&self) {
        self.unknown_usage.fetch_add(1, Ordering::Relaxed);
    }

    #[must_use]
    pub fn snapshot(&self) -> CapacitySnapshot {
        CapacitySnapshot {
            admission_wait_millis: self.admission_wait_millis.load(Ordering::Relaxed),
            admission_wait_samples: self.admission_wait_samples.load(Ordering::Relaxed),
            admission_rejected: self.admission_rejected.load(Ordering::Relaxed),
            active_capacity: self.active_capacity.load(Ordering::Relaxed),
            backpressure: self.backpressure.load(Ordering::Relaxed),
            recovery: self.recovery.load(Ordering::Relaxed),
            reconciliation: self.reconciliation.load(Ordering::Relaxed),
            provider_fallback: self.provider_fallback.load(Ordering::Relaxed),
            unknown_usage: self.unknown_usage.load(Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_is_bounded_and_keeps_unknown_usage_distinct() {
        let observations = CapacityObservations::default();
        observations.record_admission_wait(25);
        observations.record_admission_rejected();
        observations.set_active_capacity(2);
        observations.record_backpressure();
        observations.record_recovery();
        observations.record_reconciliation();
        observations.record_provider_fallback();
        observations.record_unknown_usage();
        assert_eq!(
            observations.snapshot(),
            CapacitySnapshot {
                admission_wait_millis: 25,
                admission_wait_samples: 1,
                admission_rejected: 1,
                active_capacity: 2,
                backpressure: 1,
                recovery: 1,
                reconciliation: 1,
                provider_fallback: 1,
                unknown_usage: 1,
            }
        );
    }
}
