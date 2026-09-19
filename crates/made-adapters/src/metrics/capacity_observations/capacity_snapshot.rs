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
