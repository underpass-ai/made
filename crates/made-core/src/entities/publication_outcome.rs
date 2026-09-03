use crate::entities::PublishedCeremonyDefinition;
use crate::value_objects::CeremonyDefinitionDigest;

/// Result of idempotently publishing an immutable ceremony definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublicationOutcome {
    Published(PublishedCeremonyDefinition),
    AlreadyPublished(PublishedCeremonyDefinition),
    VersionOccupied {
        published: CeremonyDefinitionDigest,
        offered: CeremonyDefinitionDigest,
    },
}

impl PublicationOutcome {
    #[must_use]
    pub fn published(&self) -> Option<&PublishedCeremonyDefinition> {
        match self {
            Self::Published(published) | Self::AlreadyPublished(published) => Some(published),
            Self::VersionOccupied { .. } => None,
        }
    }

    #[must_use]
    pub fn is_conflict(&self) -> bool {
        matches!(self, Self::VersionOccupied { .. })
    }

    #[must_use]
    pub fn is_new(&self) -> bool {
        matches!(self, Self::Published(_))
    }
}
