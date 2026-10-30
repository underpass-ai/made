use crate::entities::PublishedProtocolDefinition;
use crate::value_objects::ProtocolDefinitionDigest;

/// Result of idempotently publishing an immutable protocol definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublicationOutcome {
    Published(PublishedProtocolDefinition),
    AlreadyPublished(PublishedProtocolDefinition),
    VersionOccupied {
        published: ProtocolDefinitionDigest,
        offered: ProtocolDefinitionDigest,
    },
}

impl PublicationOutcome {
    #[must_use]
    pub fn published(&self) -> Option<&PublishedProtocolDefinition> {
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
