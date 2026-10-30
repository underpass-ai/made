use made_core::value_objects::ProtocolId;

use super::ProtocolWorkerRootPolicy;

/// Resolves trusted scheduling policy by authoritative protocol root.
pub trait ProtocolWorkerRootPolicyPort: Send + Sync {
    fn policy_for(&self, root: &ProtocolId) -> ProtocolWorkerRootPolicy;
}
