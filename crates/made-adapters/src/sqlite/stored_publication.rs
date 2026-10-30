use made_core::entities::{ProtocolDefinition, PublishedProtocolDefinition};
use made_core::error::DomainError;
use made_core::value_objects::ProtocolDefinitionDigest;
use serde::{Deserialize, Serialize};

/// A published definition and the digest it was sealed with.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(in crate::sqlite) struct StoredPublication {
    pub(super) definition: ProtocolDefinition,
    pub(super) digest: ProtocolDefinitionDigest,
}

impl StoredPublication {
    pub(super) fn seal(published: &PublishedProtocolDefinition) -> Self {
        Self {
            definition: published.definition().clone(),
            digest: published.digest(),
        }
    }

    /// The publication, verified under the digest scheme it was sealed
    /// with; see [`PublishedProtocolDefinition::verify`].
    pub(super) fn restore(self) -> Result<PublishedProtocolDefinition, DomainError> {
        PublishedProtocolDefinition::verify(self.definition, self.digest)
    }
}
