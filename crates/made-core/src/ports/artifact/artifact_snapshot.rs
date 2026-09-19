use serde::{Deserialize, Serialize};

use super::{ArtifactIdempotencyKey, ArtifactProtectionStatus, ArtifactRecord};

/// Durable store-side protection. A clock never expires these references.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSnapshot {
    pub key: ArtifactIdempotencyKey,
    pub records: Vec<ArtifactRecord>,
    pub status: ArtifactProtectionStatus,
}

impl ArtifactSnapshot {
    #[must_use]
    pub fn protected(key: ArtifactIdempotencyKey, records: Vec<ArtifactRecord>) -> Self {
        Self {
            key,
            records,
            status: ArtifactProtectionStatus::Protected,
        }
    }

    #[must_use]
    pub fn is_released(&self) -> bool {
        self.status == ArtifactProtectionStatus::Released
    }

    pub fn release(&mut self) {
        self.status = ArtifactProtectionStatus::Released;
    }

    #[must_use]
    pub fn is_protected(&self) -> bool {
        self.status == ArtifactProtectionStatus::Protected
    }
}
