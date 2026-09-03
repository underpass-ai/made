//! [`DiffCeremonyDefinitionsUseCase`] — what changed between two
//! definitions, and whether a running session could survive it.
//!
//! The comparison itself is domain work and lives in
//! [`CeremonyDefinitionDiff`]. What this adds is resolution: either
//! side may be a version in the catalogue or a document the caller is
//! holding, and an author's usual question — *is what I am about to
//! publish safe to adopt?* — has one of each.

use std::sync::Arc;

use made_core::entities::CeremonyDefinition;
use made_core::error::DomainError;
use made_core::ports::CeremonyDefinitionPublicationPort;
use made_core::value_objects::CeremonyDefinitionDiff;

use super::ceremony_definition_source::CeremonyDefinitionSource;

pub struct DiffCeremonyDefinitionsUseCase {
    publications: Arc<dyn CeremonyDefinitionPublicationPort>,
}

impl std::fmt::Debug for DiffCeremonyDefinitionsUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiffCeremonyDefinitionsUseCase").finish()
    }
}

impl DiffCeremonyDefinitionsUseCase {
    #[must_use]
    pub fn new(publications: Arc<dyn CeremonyDefinitionPublicationPort>) -> Self {
        Self { publications }
    }

    #[tracing::instrument(name = "diff_ceremony_definitions", skip_all)]
    pub async fn execute(
        &self,
        before: CeremonyDefinitionSource,
        after: CeremonyDefinitionSource,
    ) -> Result<CeremonyDefinitionDiff, DomainError> {
        let before = self.resolve(before).await?;
        let after = self.resolve(after).await?;
        Ok(CeremonyDefinitionDiff::between(&before, &after))
    }

    async fn resolve(
        &self,
        source: CeremonyDefinitionSource,
    ) -> Result<CeremonyDefinition, DomainError> {
        match source {
            CeremonyDefinitionSource::Supplied(definition) => Ok(*definition),
            // Only the catalogue, never the repository: a comparison
            // against "what is published" that quietly fell back to a
            // mounted document would answer a different question than
            // the one asked.
            CeremonyDefinitionSource::Published { name, version } => self
                .publications
                .published(&name, &version)
                .await?
                .map(made_core::entities::PublishedCeremonyDefinition::into_definition)
                .ok_or(DomainError::NotFound {
                    what: "published_ceremony_definition",
                }),
        }
    }
}
