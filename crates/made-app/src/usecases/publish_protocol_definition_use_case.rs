//! [`PublishProtocolDefinitionUseCase`] — fix a definition to an
//! immutable version.

use std::sync::Arc;

use made_core::entities::{ProtocolDefinition, PublicationOutcome, PublishedProtocolDefinition};
use made_core::error::DomainError;
use made_core::ports::ProtocolDefinitionPublicationPort;

pub struct PublishProtocolDefinitionUseCase {
    publications: Arc<dyn ProtocolDefinitionPublicationPort>,
}

impl std::fmt::Debug for PublishProtocolDefinitionUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PublishProtocolDefinitionUseCase").finish()
    }
}

impl PublishProtocolDefinitionUseCase {
    #[must_use]
    pub fn new(publications: Arc<dyn ProtocolDefinitionPublicationPort>) -> Self {
        Self { publications }
    }

    /// A `ProtocolDefinition` cannot exist while invalid, so validation
    /// is not repeated here: whatever reaches this point already
    /// satisfies every structural rule. What publication adds is
    /// identity and immutability.
    #[tracing::instrument(
        name = "publish_protocol_definition",
        skip_all,
        fields(protocol = %definition.name(), version = %definition.version())
    )]
    pub async fn execute(
        &self,
        definition: ProtocolDefinition,
    ) -> Result<PublicationOutcome, DomainError> {
        let sealed = PublishedProtocolDefinition::seal(definition)?;
        self.publications.publish(sealed).await
    }
}
