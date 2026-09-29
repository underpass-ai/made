//! [`PublishedCeremonyDefinitionQuery`] — which published definitions a
//! listing wants, and how many of them.

use made_core::entities::PublishedCeremonyDefinition;
use made_core::value_objects::{
    CeremonyDefinitionCursor, CeremonyDefinitionPageLimit, CeremonyName,
};

/// A page request over the published catalogue.
///
/// The catalogue is ordered by name and then by version. The cursor is
/// the last entry the previous page returned, so resuming never repeats
/// one and never skips one, whatever is published in between.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PublishedCeremonyDefinitionQuery {
    ceremony: Option<CeremonyName>,
    limit: CeremonyDefinitionPageLimit,
    after: Option<CeremonyDefinitionCursor>,
}

impl PublishedCeremonyDefinitionQuery {
    #[must_use]
    pub const fn new(
        ceremony: Option<CeremonyName>,
        limit: CeremonyDefinitionPageLimit,
        after: Option<CeremonyDefinitionCursor>,
    ) -> Self {
        Self {
            ceremony,
            limit,
            after,
        }
    }

    /// Only the versions published under this name, when set.
    #[must_use]
    pub const fn ceremony(&self) -> Option<&CeremonyName> {
        self.ceremony.as_ref()
    }

    #[must_use]
    pub const fn limit(&self) -> CeremonyDefinitionPageLimit {
        self.limit
    }

    #[must_use]
    pub const fn after(&self) -> Option<&CeremonyDefinitionCursor> {
        self.after.as_ref()
    }

    /// Whether this published definition belongs in the answer.
    #[must_use]
    pub fn admits(&self, published: &PublishedCeremonyDefinition) -> bool {
        self.ceremony
            .as_ref()
            .is_none_or(|wanted| wanted == published.name())
            && self
                .after
                .as_ref()
                .is_none_or(|after| after.precedes(published.name(), published.version()))
    }
}
