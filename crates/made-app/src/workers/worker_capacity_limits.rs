use super::ProtocolWorkerCapacity;

/// Limits count active operations, not starts in the current scheduling pass.
#[derive(Debug, Clone, Copy)]
pub struct WorkerCapacityLimits {
    pub global: ProtocolWorkerCapacity,
    pub per_root: ProtocolWorkerCapacity,
    pub per_connector: ProtocolWorkerCapacity,
    pub per_provider: ProtocolWorkerCapacity,
}
