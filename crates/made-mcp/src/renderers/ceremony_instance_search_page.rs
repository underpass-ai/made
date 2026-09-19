use serde_json::{json, Value};

use super::CeremonyInstanceListingEntry;

/// A bounded ceremony search result with an opaque continuation cursor.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CeremonyInstanceSearchPage {
    entries: Vec<CeremonyInstanceListingEntry>,
    next_cursor: Option<String>,
}

impl CeremonyInstanceSearchPage {
    #[must_use]
    pub(crate) fn new(
        entries: Vec<CeremonyInstanceListingEntry>,
        next_cursor: Option<String>,
    ) -> Self {
        Self {
            entries,
            next_cursor,
        }
    }

    #[must_use]
    pub(crate) fn to_json(&self) -> Value {
        let instances = self
            .entries
            .iter()
            .map(CeremonyInstanceListingEntry::to_json)
            .collect::<Vec<_>>();
        json!({
            "count": instances.len(),
            "instances": instances,
            "next_cursor": self.next_cursor,
        })
    }
}
