use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::DomainError;
use crate::value_objects::{CeremonyId, GlobalPosition};

use super::AttentionKind;

const MAX_LEN: usize = 512;

/// Stable identity of one projected reason to look at a ceremony.
///
/// Derived from the record that produced it, so replaying the feed
/// after a restart produces the same identity and the ledger recognises
/// the item it already holds instead of delivering it twice.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct AttentionEventId(String);

impl AttentionEventId {
    /// Validate an identifier read back from storage or a host payload.
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let raw = raw.into();
        let value = raw.trim();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "attention_event_id",
            });
        }
        if value.len() > MAX_LEN {
            return Err(DomainError::FieldTooLong {
                field: "attention_event_id",
                actual: value.len(),
                max: MAX_LEN,
            });
        }
        if value.chars().any(char::is_control) {
            return Err(DomainError::InvalidCharacters {
                field: "attention_event_id",
            });
        }
        Ok(Self(value.to_owned()))
    }

    /// The identity the projection deduplicates on.
    ///
    /// The position is in it because the ledger holds identities and
    /// the loop has to hand a host the event itself. Carrying where the
    /// record sits turns that into one bounded read at a known
    /// position, instead of a scan for an event id the store has no
    /// index for. It is as stable as the rest: a record's place in the
    /// global order does not move, so replaying the feed still derives
    /// the same identity.
    pub fn derive(
        ceremony_id: &CeremonyId,
        position: GlobalPosition,
        source_event_id: &str,
        kind: AttentionKind,
    ) -> Result<Self, DomainError> {
        Self::new(format!(
            "{ceremony_id}:{}:{source_event_id}:{}",
            position.value(),
            kind.as_str()
        ))
    }

    /// Where in the global feed the record behind this sits.
    ///
    /// The ceremony has to be supplied because a ceremony identifier
    /// may itself contain a colon, so the prefix is known rather than
    /// guessed. `None` means the identity was not derived by this
    /// build — an identifier read back from a host, say — and the
    /// caller has to go and look the ordinary way.
    #[must_use]
    pub fn source_position(&self, ceremony_id: &CeremonyId) -> Option<GlobalPosition> {
        let rest = self.0.strip_prefix(&format!("{ceremony_id}:"))?;
        let (position, _) = rest.split_once(':')?;
        position
            .parse::<u64>()
            .ok()
            .and_then(|value| GlobalPosition::new(value).ok())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AttentionEventId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for AttentionEventId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(value: u64) -> GlobalPosition {
        GlobalPosition::new(value).unwrap()
    }

    #[test]
    fn one_record_and_one_kind_derive_one_identity() {
        let ceremony = CeremonyId::new("c-1").unwrap();
        let first =
            AttentionEventId::derive(&ceremony, at(9), "e-9", AttentionKind::ResultAvailable);
        let second =
            AttentionEventId::derive(&ceremony, at(9), "e-9", AttentionKind::ResultAvailable);
        assert_eq!(first.unwrap(), second.unwrap());
    }

    #[test]
    fn one_record_can_ask_for_attention_twice_for_different_reasons() {
        let ceremony = CeremonyId::new("c-1").unwrap();
        let result =
            AttentionEventId::derive(&ceremony, at(9), "e-9", AttentionKind::ResultAvailable);
        let rejected =
            AttentionEventId::derive(&ceremony, at(9), "e-9", AttentionKind::ReviewRejected);
        assert_ne!(result.unwrap(), rejected.unwrap());
    }

    #[test]
    fn the_identity_says_where_to_go_and_read_the_record() {
        let ceremony = CeremonyId::new("c-1").unwrap();
        let id = AttentionEventId::derive(&ceremony, at(42), "e-9", AttentionKind::ResultAvailable)
            .unwrap();

        assert_eq!(id.source_position(&ceremony), Some(at(42)));
    }

    #[test]
    fn a_ceremony_whose_name_has_a_colon_still_reads_back() {
        let ceremony = CeremonyId::new("run:1").unwrap();
        let id = AttentionEventId::derive(&ceremony, at(7), "e-9", AttentionKind::CeremonyEnded)
            .unwrap();

        assert_eq!(
            id.source_position(&ceremony),
            Some(at(7)),
            "the prefix is known, not guessed, precisely so this works"
        );
    }

    #[test]
    fn an_identity_from_somewhere_else_admits_it_does_not_know() {
        let ceremony = CeremonyId::new("c-1").unwrap();
        let handed_back = AttentionEventId::new("something a host made up").unwrap();

        assert!(handed_back.source_position(&ceremony).is_none());
    }
}
