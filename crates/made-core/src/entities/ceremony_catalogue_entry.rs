//! [`CeremonyCatalogueEntry`] — one row of the published catalogue, as
//! a listing sees it.

use crate::value_objects::{CeremonyName, CeremonyVersion};

use super::{PublishedCeremonyDefinition, UnreadableCeremonyPublication};

/// A publication this engine can read back, or one it cannot and says
/// why. Listing the catalogue answers with both, so one damaged row
/// does not take every readable one with it.
#[derive(Debug, Clone, PartialEq)]
pub enum CeremonyCatalogueEntry {
    // Boxed: a definition is several times the size of the reason a
    // row could not be read.
    Readable(Box<PublishedCeremonyDefinition>),
    Unreadable(UnreadableCeremonyPublication),
}

impl CeremonyCatalogueEntry {
    #[must_use]
    pub fn name(&self) -> &CeremonyName {
        match self {
            Self::Readable(published) => published.name(),
            Self::Unreadable(unreadable) => unreadable.name(),
        }
    }

    #[must_use]
    pub fn version(&self) -> &CeremonyVersion {
        match self {
            Self::Readable(published) => published.version(),
            Self::Unreadable(unreadable) => unreadable.version(),
        }
    }
}
