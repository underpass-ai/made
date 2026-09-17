use serde_json::{json, Value};

/// The verdict on one journal's chain, as both backends answer it.
///
/// One renderer, not two. The sealed records already go out through a
/// presenter per arm — one over the in-process types, one over the
/// proto message — and keeping the verdict's shape in two places would
/// be the third chance for the arms to disagree about a field name.
/// Each arm fills this from what it has; the JSON is written once.
///
/// Absent is `null` here, where the contract has to say it with a zero
/// and an empty string: a caller reading this JSON tests one field
/// rather than knowing that position zero means "no position".
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CeremonyJournalVerdictView {
    pub(crate) ceremony_id: String,
    pub(crate) head_version: u64,
    pub(crate) record_count: u64,
    pub(crate) intact: bool,
    pub(crate) first_broken_sequence: Option<u64>,
    pub(crate) reason: Option<String>,
}

impl CeremonyJournalVerdictView {
    pub(crate) fn to_json(&self) -> Value {
        json!({
            "ceremony_id": self.ceremony_id,
            "head_version": self.head_version,
            "record_count": self.record_count,
            "intact": self.intact,
            "first_broken_sequence": self.first_broken_sequence,
            "reason": self.reason,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn intact() -> CeremonyJournalVerdictView {
        CeremonyJournalVerdictView {
            ceremony_id: "session-1".to_owned(),
            head_version: 4,
            record_count: 4,
            intact: true,
            first_broken_sequence: None,
            reason: None,
        }
    }

    #[test]
    fn an_intact_journal_says_so_with_nulls_rather_than_zeroes() {
        let json = intact().to_json();

        assert_eq!(json["ceremony_id"], "session-1");
        assert_eq!(json["head_version"], 4);
        assert_eq!(json["record_count"], 4);
        assert_eq!(json["intact"], true);
        assert!(json["first_broken_sequence"].is_null());
        assert!(json["reason"].is_null());
    }

    #[test]
    fn a_broken_journal_names_the_position_and_the_reason() {
        let json = CeremonyJournalVerdictView {
            intact: false,
            record_count: 3,
            first_broken_sequence: Some(2),
            reason: Some("the record at position 2 no longer produces its own digest".to_owned()),
            ..intact()
        }
        .to_json();

        assert_eq!(json["intact"], false);
        assert_eq!(json["first_broken_sequence"], 2);
        assert!(json["reason"].as_str().unwrap().contains("digest"));
    }

    /// The six keys, and no seventh: both arms render through here, so
    /// this is where a field added to one of them would have to be
    /// added.
    #[test]
    fn the_answer_has_exactly_the_six_keys_the_contract_names() {
        let json = intact().to_json();

        let mut keys: Vec<&String> = json.as_object().unwrap().keys().collect();
        keys.sort();
        assert_eq!(
            keys,
            [
                "ceremony_id",
                "first_broken_sequence",
                "head_version",
                "intact",
                "reason",
                "record_count"
            ]
        );
    }
}
