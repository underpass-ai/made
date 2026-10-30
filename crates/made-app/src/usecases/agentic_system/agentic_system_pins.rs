//! Resolving what a design's pins actually name.
//!
//! One place, used by design, validation, publication and
//! instantiation alike. Each of them asks the publication port the
//! same question, and four spellings of it would be four chances for
//! one of them to compare a version where another compared bytes.

use std::collections::BTreeMap;
use std::sync::Arc;

use made_core::entities::{AgenticSystem, PublishedProtocolDefinition};
use made_core::error::DomainError;
use made_core::ports::ProtocolDefinitionPublicationPort;
use made_core::value_objects::{ProtocolComposition, ProtocolDefinitionDigest, SystemProtocolId};

use super::AgenticSystemDesignDocument;

/// Looks pins up in the published catalogue.
pub struct AgenticSystemPins {
    publications: Arc<dyn ProtocolDefinitionPublicationPort>,
}

impl std::fmt::Debug for AgenticSystemPins {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("AgenticSystemPins").finish()
    }
}

impl AgenticSystemPins {
    #[must_use]
    pub const fn new(publications: Arc<dyn ProtocolDefinitionPublicationPort>) -> Self {
        Self { publications }
    }

    /// A digest for each composition whose author stated none.
    ///
    /// Refused, with the composition named, when nothing is published
    /// under that version: the design would otherwise have to hold a
    /// digest of nothing, and every later check would be comparing
    /// against a value nobody chose.
    pub async fn digests(
        &self,
        document: &AgenticSystemDesignDocument,
    ) -> Result<BTreeMap<SystemProtocolId, ProtocolDefinitionDigest>, DomainError> {
        let mut resolved = BTreeMap::new();
        for composition in document.compositions() {
            if composition.pin().stated_digest()?.is_some() {
                continue;
            }
            let pin = composition.pin();
            let published = self
                .publications
                .published(pin.name(), pin.version())
                .await?
                .ok_or_else(|| DomainError::InvalidDocument {
                    reason: format!(
                        "protocol `{}` composes `{}@{}`, which is not published; publish it \
                         first, or state the digest you mean",
                        composition.id(),
                        pin.name(),
                        pin.version()
                    ),
                })?;
            resolved.insert(composition.id().clone(), published.digest());
        }
        Ok(resolved)
    }

    /// The published definition behind each composition, where there
    /// is one.
    ///
    /// Looked up by name and version only. Whether the digest matches
    /// what the design pinned is the analysis's finding to make, and
    /// filtering the mismatches out here would turn an explainable
    /// refusal into an unexplained absence.
    pub async fn resolve(
        &self,
        system: &AgenticSystem,
    ) -> Result<BTreeMap<SystemProtocolId, PublishedProtocolDefinition>, DomainError> {
        let mut resolved = BTreeMap::new();
        for (id, composition) in system.protocols() {
            if let Some(published) = self.published(composition).await? {
                resolved.insert(id.clone(), published);
            }
        }
        Ok(resolved)
    }

    async fn published(
        &self,
        composition: &ProtocolComposition,
    ) -> Result<Option<PublishedProtocolDefinition>, DomainError> {
        self.publications
            .published(composition.pin().name(), composition.pin().version())
            .await
    }
}
