use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::DomainError;
use crate::value_objects::CeremonyId;

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
    pub fn derive(
        ceremony_id: &CeremonyId,
        source_event_id: &str,
        kind: AttentionKind,
    ) -> Result<Self, DomainError> {
        Self::new(format!("{ceremony_id}:{source_event_id}:{}", kind.as_str()))
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

    #[test]
    fn one_record_and_one_kind_derive_one_identity() {
        let ceremony = CeremonyId::new("c-1").unwrap();
        let first = AttentionEventId::derive(&ceremony, "e-9", AttentionKind::ResultAvailable);
        let second = AttentionEventId::derive(&ceremony, "e-9", AttentionKind::ResultAvailable);
        assert_eq!(first.unwrap(), second.unwrap());
    }

    #[test]
    fn one_record_can_ask_for_attention_twice_for_different_reasons() {
        let ceremony = CeremonyId::new("c-1").unwrap();
        let result = AttentionEventId::derive(&ceremony, "e-9", AttentionKind::ResultAvailable);
        let rejected = AttentionEventId::derive(&ceremony, "e-9", AttentionKind::ReviewRejected);
        assert_ne!(result.unwrap(), rejected.unwrap());
    }
}
