use serde_json::{json, Value};

use super::ProtocolInstanceListingEntry;

/// A bounded protocol search result with an opaque continuation cursor.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ProtocolInstanceSearchPage {
    entries: Vec<ProtocolInstanceListingEntry>,
    next_cursor: Option<String>,
}

impl ProtocolInstanceSearchPage {
    #[must_use]
    pub(crate) fn new(
        entries: Vec<ProtocolInstanceListingEntry>,
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
            .map(ProtocolInstanceListingEntry::to_json)
            .collect::<Vec<_>>();
        json!({
            "count": instances.len(),
            "instances": instances,
            "next_cursor": self.next_cursor,
        })
    }
}
