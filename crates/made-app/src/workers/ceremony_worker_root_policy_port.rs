use made_core::value_objects::CeremonyId;

use super::CeremonyWorkerRootPolicy;

/// Resolves trusted scheduling policy by authoritative ceremony root.
pub trait CeremonyWorkerRootPolicyPort: Send + Sync {
    fn policy_for(&self, root: &CeremonyId) -> CeremonyWorkerRootPolicy;
}
