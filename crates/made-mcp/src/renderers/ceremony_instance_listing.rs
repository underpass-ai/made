use serde_json::{json, Value};

use super::CeremonyInstanceListingEntry;

/// One `made_list_ceremony_instances` answer.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CeremonyInstanceListing {
    entries: Vec<CeremonyInstanceListingEntry>,
}

impl CeremonyInstanceListing {
    #[must_use]
    pub(crate) fn new(entries: Vec<CeremonyInstanceListingEntry>) -> Self {
        Self { entries }
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
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_both_listing_entries_and_the_count() {
        let rendered = CeremonyInstanceListing::new(vec![
            CeremonyInstanceListingEntry::rehydratable(json!({"ceremony_id": "one"})),
            CeremonyInstanceListingEntry::unrehydratable("two", "definition missing"),
        ])
        .to_json();

        assert_eq!(rendered["count"], 2);
        assert_eq!(rendered["instances"][0]["rehydratable"], true);
        assert_eq!(rendered["instances"][0]["reason"], Value::Null);
        assert_eq!(rendered["instances"][1]["rehydratable"], false);
        assert_eq!(rendered["instances"][1]["reason"], "definition missing");
    }
}
