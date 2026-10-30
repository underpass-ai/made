//! [`PublishedProtocolDefinitionQuery`] — which published definitions a
//! listing wants, and how many of them.

use made_core::entities::ProtocolCatalogueEntry;
use made_core::value_objects::{
    ProtocolDefinitionCursor, ProtocolDefinitionPageLimit, ProtocolName,
};

/// A page request over the published catalogue.
///
/// The catalogue is ordered by name and then by version. The cursor is
/// the last entry the previous page returned, so resuming never repeats
/// one and never skips one, whatever is published in between.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PublishedProtocolDefinitionQuery {
    protocol: Option<ProtocolName>,
    limit: ProtocolDefinitionPageLimit,
    after: Option<ProtocolDefinitionCursor>,
}

impl PublishedProtocolDefinitionQuery {
    #[must_use]
    pub const fn new(
        protocol: Option<ProtocolName>,
        limit: ProtocolDefinitionPageLimit,
        after: Option<ProtocolDefinitionCursor>,
    ) -> Self {
        Self {
            protocol,
            limit,
            after,
        }
    }

    /// Only the versions published under this name, when set.
    #[must_use]
    pub const fn protocol(&self) -> Option<&ProtocolName> {
        self.protocol.as_ref()
    }

    #[must_use]
    pub const fn limit(&self) -> ProtocolDefinitionPageLimit {
        self.limit
    }

    #[must_use]
    pub const fn after(&self) -> Option<&ProtocolDefinitionCursor> {
        self.after.as_ref()
    }

    /// Whether this catalogue entry belongs in the answer.
    #[must_use]
    pub fn admits(&self, entry: &ProtocolCatalogueEntry) -> bool {
        self.protocol
            .as_ref()
            .is_none_or(|wanted| wanted == entry.name())
            && self
                .after
                .as_ref()
                .is_none_or(|after| after.precedes(entry.name(), entry.version()))
    }
}
