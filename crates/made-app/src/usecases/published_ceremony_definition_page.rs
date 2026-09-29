//! [`PublishedCeremonyDefinitionPage`] — one page of the published
//! catalogue.

use made_core::entities::CeremonyCatalogueEntry;
use made_core::value_objects::CeremonyDefinitionCursor;

/// Catalogue entries in catalogue order, and where to continue. An
/// entry the store cannot hand back is in the page, marked, rather than
/// missing from it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PublishedCeremonyDefinitionPage {
    entries: Vec<CeremonyCatalogueEntry>,
    next_cursor: Option<CeremonyDefinitionCursor>,
}

impl PublishedCeremonyDefinitionPage {
    #[must_use]
    pub const fn new(
        entries: Vec<CeremonyCatalogueEntry>,
        next_cursor: Option<CeremonyDefinitionCursor>,
    ) -> Self {
        Self {
            entries,
            next_cursor,
        }
    }

    #[must_use]
    pub fn entries(&self) -> &[CeremonyCatalogueEntry] {
        &self.entries
    }

    /// Where the next page starts, when there is one.
    #[must_use]
    pub const fn next_cursor(&self) -> Option<&CeremonyDefinitionCursor> {
        self.next_cursor.as_ref()
    }
}
