use super::CeremonyWorkerCapacity;

/// Limits count active operations, not starts in the current scheduling pass.
#[derive(Debug, Clone, Copy)]
pub struct WorkerCapacityLimits {
    pub global: CeremonyWorkerCapacity,
    pub per_root: CeremonyWorkerCapacity,
    pub per_connector: CeremonyWorkerCapacity,
    pub per_provider: CeremonyWorkerCapacity,
}
