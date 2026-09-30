use made_core::entities::{CeremonyDefinition, PublishedCeremonyDefinition};
use made_core::error::DomainError;
use made_core::value_objects::CeremonyDefinitionDigest;
use serde::{Deserialize, Serialize};

/// A published definition and the digest it was sealed with.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(in crate::sqlite) struct StoredPublication {
    pub(super) definition: CeremonyDefinition,
    pub(super) digest: CeremonyDefinitionDigest,
}

impl StoredPublication {
    pub(super) fn seal(published: &PublishedCeremonyDefinition) -> Self {
        Self {
            definition: published.definition().clone(),
            digest: published.digest(),
        }
    }

    /// The publication, verified under the digest scheme it was sealed
    /// with; see [`PublishedCeremonyDefinition::verify`].
    pub(super) fn restore(self) -> Result<PublishedCeremonyDefinition, DomainError> {
        PublishedCeremonyDefinition::verify(self.definition, self.digest)
    }
}
