//! [`PublishedProtocolDefinitionPage`] — one page of the published
//! catalogue.

use made_core::entities::ProtocolCatalogueEntry;
use made_core::value_objects::ProtocolDefinitionCursor;

/// Catalogue entries in catalogue order, and where to continue. An
/// entry the store cannot hand back is in the page, marked, rather than
/// missing from it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PublishedProtocolDefinitionPage {
    entries: Vec<ProtocolCatalogueEntry>,
    next_cursor: Option<ProtocolDefinitionCursor>,
}

impl PublishedProtocolDefinitionPage {
    #[must_use]
    pub const fn new(
        entries: Vec<ProtocolCatalogueEntry>,
        next_cursor: Option<ProtocolDefinitionCursor>,
    ) -> Self {
        Self {
            entries,
            next_cursor,
        }
    }

    #[must_use]
    pub fn entries(&self) -> &[ProtocolCatalogueEntry] {
        &self.entries
    }

    /// Where the next page starts, when there is one.
    #[must_use]
    pub const fn next_cursor(&self) -> Option<&ProtocolDefinitionCursor> {
        self.next_cursor.as_ref()
    }
}
