use crate::value_objects::AgenticSystemDigest;

use super::PublishedAgenticSystem;

/// Result of idempotently sealing one revision of a design.
///
/// The same shape as a ceremony definition's publication outcome, and
/// for the same reason: offering identical content under a revision
/// that already holds it is not an error, offering different content
/// under it is, and both digests come back so the caller can see what
/// differed rather than being told only that something did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgenticSystemPublicationOutcome {
    Published(PublishedAgenticSystem),
    AlreadyPublished(PublishedAgenticSystem),
    RevisionOccupied {
        published: AgenticSystemDigest,
        offered: AgenticSystemDigest,
    },
}

impl AgenticSystemPublicationOutcome {
    #[must_use]
    pub const fn published(&self) -> Option<&PublishedAgenticSystem> {
        match self {
            Self::Published(published) | Self::AlreadyPublished(published) => Some(published),
            Self::RevisionOccupied { .. } => None,
        }
    }

    #[must_use]
    pub const fn is_conflict(&self) -> bool {
        matches!(self, Self::RevisionOccupied { .. })
    }

    #[must_use]
    pub const fn is_new(&self) -> bool {
        matches!(self, Self::Published(_))
    }

    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Published(_) => "published",
            Self::AlreadyPublished(_) => "already_published",
            Self::RevisionOccupied { .. } => "revision_occupied",
        }
    }
}
