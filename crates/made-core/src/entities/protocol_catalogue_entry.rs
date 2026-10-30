//! [`ProtocolCatalogueEntry`] — one row of the published catalogue, as
//! a listing sees it.

use crate::value_objects::{ProtocolName, ProtocolVersion};

use super::{PublishedProtocolDefinition, UnreadableProtocolPublication};

/// A publication this engine can read back, or one it cannot and says
/// why. Listing the catalogue answers with both, so one damaged row
/// does not take every readable one with it.
#[derive(Debug, Clone, PartialEq)]
pub enum ProtocolCatalogueEntry {
    // Boxed: a definition is several times the size of the reason a
    // row could not be read.
    Readable(Box<PublishedProtocolDefinition>),
    Unreadable(UnreadableProtocolPublication),
}

impl ProtocolCatalogueEntry {
    #[must_use]
    pub fn name(&self) -> &ProtocolName {
        match self {
            Self::Readable(published) => published.name(),
            Self::Unreadable(unreadable) => unreadable.name(),
        }
    }

    #[must_use]
    pub fn version(&self) -> &ProtocolVersion {
        match self {
            Self::Readable(published) => published.version(),
            Self::Unreadable(unreadable) => unreadable.version(),
        }
    }
}
