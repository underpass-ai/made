//! [`ResolveProtocolDefinitionUseCase`] — the definition an instance
//! actually runs.
//!
//! Shared by every distribution on purpose. Resolving a binding is a
//! rule about what a working session *is*, so a server that
//! reimplemented it would be aligned with the embedded engine only
//! until one of the two changed.

use std::sync::Arc;

use made_core::entities::{ProtocolDefinition, ProtocolInstance};
use made_core::error::DomainError;
use made_core::ports::{ProtocolDefinitionPublicationPort, ProtocolDefinitionRepositoryPort};
use made_core::value_objects::DefinitionPin;

pub struct ResolveProtocolDefinitionUseCase {
    definitions: Arc<dyn ProtocolDefinitionRepositoryPort>,
    publications: Arc<dyn ProtocolDefinitionPublicationPort>,
}

impl std::fmt::Debug for ResolveProtocolDefinitionUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolveProtocolDefinitionUseCase").finish()
    }
}

impl ResolveProtocolDefinitionUseCase {
    #[must_use]
    pub fn new(
        definitions: Arc<dyn ProtocolDefinitionRepositoryPort>,
        publications: Arc<dyn ProtocolDefinitionPublicationPort>,
    ) -> Self {
        Self {
            definitions,
            publications,
        }
    }

    /// A bound instance is resolved from the published catalogue and
    /// **checked against the digest it recorded**. That check is the
    /// reason for storing the digest: without it, a name and a version
    /// are a promise that whatever answers to them today is what ran,
    /// and a reader has no way to tell when it is not.
    ///
    /// An unbound instance resolves from the repository, where nothing
    /// can be checked — the honest difference between the two ways of
    /// starting a working session.
    pub async fn execute(
        &self,
        instance: &ProtocolInstance,
    ) -> Result<ProtocolDefinition, DomainError> {
        Ok(self.execute_pinned(instance).await?.0)
    }

    /// The definition and the pin that names it.
    ///
    /// A bound instance is pinned by the digest its publication was
    /// sealed with — the one the instance recorded — never by a digest
    /// recomputed under the current scheme, which differs for a
    /// publication sealed under an earlier one (ADR 023). An unbound
    /// instance has no publication, so its pin is the content digest
    /// under the current scheme.
    pub async fn execute_pinned(
        &self,
        instance: &ProtocolInstance,
    ) -> Result<(ProtocolDefinition, DefinitionPin), DomainError> {
        let Some(digest) = instance.bound_definition() else {
            let definition = self
                .definitions
                .get(instance.definition_name(), instance.definition_version())
                .await?;
            let pin = DefinitionPin::new(
                definition.name().clone(),
                definition.version().clone(),
                definition.digest()?,
            );
            return Ok((definition, pin));
        };

        let published = self
            .publications
            .published(instance.definition_name(), instance.definition_version())
            .await?
            .ok_or(DomainError::NotFound {
                what: "published_protocol_definition",
            })?;
        if published.digest() != digest {
            return Err(DomainError::InvariantViolated {
                reason: "the published definition no longer matches the digest this instance ran",
            });
        }
        let pin = DefinitionPin::new(
            published.name().clone(),
            published.version().clone(),
            published.digest(),
        );
        Ok((published.into_definition(), pin))
    }
}
