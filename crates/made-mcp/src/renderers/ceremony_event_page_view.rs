use serde_json::{json, Value};

use super::AuditRecordView;

/// One bounded page of sealed records at a named stream head.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CeremonyEventPageView {
    records: Vec<AuditRecordView>,
    next_version: u64,
    head_version: u64,
}

impl CeremonyEventPageView {
    #[must_use]
    pub(crate) fn new(records: Vec<AuditRecordView>, next_version: u64, head_version: u64) -> Self {
        Self {
            records,
            next_version,
            head_version,
        }
    }

    /// Render count and continuation metadata from the same record collection.
    #[must_use]
    pub(crate) fn to_json(&self) -> Value {
        json!({
            "records": self
                .records
                .iter()
                .map(AuditRecordView::to_json)
                .collect::<Vec<_>>(),
            "record_count": self.records.len(),
            "next_version": self.next_version,
            "head_version": self.head_version,
            "has_more": self.next_version < self.head_version,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_count_and_has_more_from_one_page() {
        let rendered = CeremonyEventPageView::new(Vec::new(), 2, 3).to_json();

        assert_eq!(rendered["records"], json!([]));
        assert_eq!(rendered["record_count"], 0);
        assert_eq!(rendered["has_more"], true);
    }
}
