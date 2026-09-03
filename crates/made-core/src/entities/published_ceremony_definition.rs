//! [`PublishedCeremonyDefinition`] — a definition fixed to a content
//! identity.
//!
//! An agent can already write a definition and run it. What publication
//! adds is that the thing it ran can be named later and shown to be the
//! same thing: an immutable version with a digest an instance binds to
//! and an auditor recomputes.
//!
//! Running an ad-hoc definition stays possible and is not the same act.
//! Investigation should not need a published version; governed reuse
//! should not accept an unpublished one.

use crate::error::DomainError;
use crate::value_objects::{CeremonyDefinitionDigest, CeremonyName, CeremonyVersion};

use super::CeremonyDefinition;

/// A definition and the digest that identifies its content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedCeremonyDefinition {
    definition: CeremonyDefinition,
    digest: CeremonyDefinitionDigest,
}

impl PublishedCeremonyDefinition {
    /// Fix a definition to its content identity.
    ///
    /// Only a definition can be sealed, and a `CeremonyDefinition`
    /// cannot exist while invalid — so an unpublishable draft can never
    /// reach this constructor.
    pub fn seal(definition: CeremonyDefinition) -> Result<Self, DomainError> {
        let digest = definition.digest()?;
        Ok(Self { definition, digest })
    }

    #[must_use]
    pub fn definition(&self) -> &CeremonyDefinition {
        &self.definition
    }

    #[must_use]
    pub fn digest(&self) -> CeremonyDefinitionDigest {
        self.digest
    }

    #[must_use]
    pub fn name(&self) -> &CeremonyName {
        self.definition.name()
    }

    #[must_use]
    pub fn version(&self) -> &CeremonyVersion {
        self.definition.version()
    }

    #[must_use]
    pub fn into_definition(self) -> CeremonyDefinition {
        self.definition
    }
}
