//! [`PublishedCeremonyDefinitionPage`] — one page of the published
//! catalogue.

use made_core::entities::PublishedCeremonyDefinition;
use made_core::value_objects::CeremonyDefinitionCursor;

/// Published definitions in catalogue order, and where to continue.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PublishedCeremonyDefinitionPage {
    definitions: Vec<PublishedCeremonyDefinition>,
    next_cursor: Option<CeremonyDefinitionCursor>,
}

impl PublishedCeremonyDefinitionPage {
    #[must_use]
    pub const fn new(
        definitions: Vec<PublishedCeremonyDefinition>,
        next_cursor: Option<CeremonyDefinitionCursor>,
    ) -> Self {
        Self {
            definitions,
            next_cursor,
        }
    }

    #[must_use]
    pub fn definitions(&self) -> &[PublishedCeremonyDefinition] {
        &self.definitions
    }

    /// Where the next page starts, when there is one.
    #[must_use]
    pub const fn next_cursor(&self) -> Option<&CeremonyDefinitionCursor> {
        self.next_cursor.as_ref()
    }
}
