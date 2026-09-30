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
use crate::value_objects::{
    CeremonyDefinitionDigest, CeremonyDefinitionDigestScheme, CeremonyName, CeremonyVersion,
};

use super::CeremonyDefinition;

/// A definition and the digest that identifies its content.
///
/// The digest is the one the publication was sealed with, and the scheme
/// says which digest algorithm produced it. Only a publication read back
/// from a store can carry a scheme other than
/// [`CeremonyDefinitionDigestScheme::CURRENT`]; see [`Self::verify`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedCeremonyDefinition {
    definition: CeremonyDefinition,
    digest: CeremonyDefinitionDigest,
    scheme: CeremonyDefinitionDigestScheme,
}

impl PublishedCeremonyDefinition {
    /// Fix a definition to its content identity.
    ///
    /// Only a definition can be sealed, and a `CeremonyDefinition`
    /// cannot exist while invalid — so an unpublishable draft can never
    /// reach this constructor.
    pub fn seal(definition: CeremonyDefinition) -> Result<Self, DomainError> {
        let digest = definition.digest()?;
        Ok(Self {
            definition,
            digest,
            scheme: CeremonyDefinitionDigestScheme::CURRENT,
        })
    }

    /// Restore a stored publication, proving its recorded digest.
    ///
    /// The recorded digest is recomputed from the definition under every
    /// [known scheme](CeremonyDefinitionDigestScheme::KNOWN), and the
    /// publication keeps the digest it was sealed with — instances bound
    /// to it recorded that digest, so resealing would orphan them. A
    /// digest matching no known scheme is refused: that is content which
    /// no longer matches its identity, whatever produced it.
    pub fn verify(
        definition: CeremonyDefinition,
        recorded: CeremonyDefinitionDigest,
    ) -> Result<Self, DomainError> {
        for scheme in CeremonyDefinitionDigestScheme::KNOWN {
            if definition.digest_under(scheme)? == recorded {
                return Ok(Self {
                    definition,
                    digest: recorded,
                    scheme,
                });
            }
        }
        Err(DomainError::InvariantViolated {
            reason: "the stored publication digest does not match its definition",
        })
    }

    /// Whether `offered` publishes the same content as this publication,
    /// whichever known scheme either was sealed under.
    ///
    /// Digests under different schemes never compare equal, so a
    /// republication of unchanged content over a row sealed under an
    /// earlier scheme is recognised by recomputing this content under
    /// the offer's scheme.
    pub fn holds_same_content_as(&self, offered: &Self) -> Result<bool, DomainError> {
        if self.scheme == offered.scheme {
            return Ok(self.digest == offered.digest);
        }
        Ok(self.definition.digest_under(offered.scheme)? == offered.digest)
    }

    #[must_use]
    pub fn definition(&self) -> &CeremonyDefinition {
        &self.definition
    }

    #[must_use]
    pub fn digest(&self) -> CeremonyDefinitionDigest {
        self.digest
    }

    /// The digest algorithm [`Self::digest`] was computed with.
    #[must_use]
    pub fn scheme(&self) -> CeremonyDefinitionDigestScheme {
        self.scheme
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
