//! [`DiffProtocolDefinitionsUseCase`] — what changed between two
//! definitions, and whether a running session could survive it.
//!
//! The comparison itself is domain work and lives in
//! [`ProtocolDefinitionDiff`]. What this adds is resolution: either
//! side may be a version in the catalogue or a document the caller is
//! holding, and an author's usual question — *is what I am about to
//! publish safe to adopt?* — has one of each.

use std::sync::Arc;

use made_core::entities::ProtocolDefinition;
use made_core::error::DomainError;
use made_core::ports::ProtocolDefinitionPublicationPort;
use made_core::value_objects::ProtocolDefinitionDiff;

use super::protocol_definition_source::ProtocolDefinitionSource;

pub struct DiffProtocolDefinitionsUseCase {
    publications: Arc<dyn ProtocolDefinitionPublicationPort>,
}

impl std::fmt::Debug for DiffProtocolDefinitionsUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiffProtocolDefinitionsUseCase").finish()
    }
}

impl DiffProtocolDefinitionsUseCase {
    #[must_use]
    pub fn new(publications: Arc<dyn ProtocolDefinitionPublicationPort>) -> Self {
        Self { publications }
    }

    #[tracing::instrument(name = "diff_protocol_definitions", skip_all)]
    pub async fn execute(
        &self,
        before: ProtocolDefinitionSource,
        after: ProtocolDefinitionSource,
    ) -> Result<ProtocolDefinitionDiff, DomainError> {
        let before = self.resolve(before).await?;
        let after = self.resolve(after).await?;
        Ok(ProtocolDefinitionDiff::between(&before, &after))
    }

    async fn resolve(
        &self,
        source: ProtocolDefinitionSource,
    ) -> Result<ProtocolDefinition, DomainError> {
        match source {
            ProtocolDefinitionSource::Supplied(definition) => Ok(*definition),
            // Only the catalogue, never the repository: a comparison
            // against "what is published" that quietly fell back to a
            // mounted document would answer a different question than
            // the one asked.
            ProtocolDefinitionSource::Published { name, version } => self
                .publications
                .published(&name, &version)
                .await?
                .map(made_core::entities::PublishedProtocolDefinition::into_definition)
                .ok_or(DomainError::NotFound {
                    what: "published_protocol_definition",
                }),
        }
    }
}
