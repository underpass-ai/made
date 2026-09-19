use made_core::ports::ArtifactIdempotencyKey;
use made_core::value_objects::ArtifactRef;

/// Durable audit artifact and the key used to protect it from collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionReconciliationAuditRecord {
    artifact: ArtifactRef,
    protection_key: ArtifactIdempotencyKey,
}

impl ExecutionReconciliationAuditRecord {
    #[must_use]
    pub const fn new(artifact: ArtifactRef, protection_key: ArtifactIdempotencyKey) -> Self {
        Self {
            artifact,
            protection_key,
        }
    }

    #[must_use]
    pub const fn artifact(&self) -> &ArtifactRef {
        &self.artifact
    }

    #[must_use]
    pub const fn protection_key(&self) -> &ArtifactIdempotencyKey {
        &self.protection_key
    }
}
