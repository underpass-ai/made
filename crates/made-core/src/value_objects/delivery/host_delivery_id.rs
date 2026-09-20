use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::DomainError;
use crate::value_objects::CeremonyId;

use super::{HostDeliveryItemKind, HostDeliveryTargetKey};

const MAX_LEN: usize = 512;

/// Stable identity of one item's delivery to one host destination.
///
/// Derived rather than generated, because the ledger's deduplication is
/// the identity: the same item offered to the same destination twice is
/// one delivery, whoever enqueued it and however many times a projector
/// replays the record that produced it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct HostDeliveryId(String);

impl HostDeliveryId {
    /// Validate an identifier read back from storage or a host payload.
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let raw = raw.into();
        let value = raw.trim();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "host_delivery_id",
            });
        }
        if value.len() > MAX_LEN {
            return Err(DomainError::FieldTooLong {
                field: "host_delivery_id",
                actual: value.len(),
                max: MAX_LEN,
            });
        }
        if value.chars().any(char::is_control) {
            return Err(DomainError::InvalidCharacters {
                field: "host_delivery_id",
            });
        }
        Ok(Self(value.to_owned()))
    }

    /// The identity the ledger deduplicates on.
    pub fn derive(
        ceremony_id: &CeremonyId,
        item_kind: HostDeliveryItemKind,
        item_id: &str,
        target_key: &HostDeliveryTargetKey,
    ) -> Result<Self, DomainError> {
        Self::new(format!(
            "{ceremony_id}:{}:{item_id}:{target_key}",
            item_kind.as_str()
        ))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for HostDeliveryId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

// Deserialisation goes through the constructor so a stored or host-supplied
// identifier cannot reconstitute one the constructor would have refused.
impl<'de> Deserialize<'de> for HostDeliveryId {
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
    use crate::value_objects::{HostAgentIncarnation, HostDeliveryTarget};

    #[test]
    fn the_same_item_and_destination_derive_one_identity() {
        let ceremony = CeremonyId::new("c-1").unwrap();
        let target = HostDeliveryTarget::agent_execution(
            crate::value_objects::CeremonyAgentExecutionId::new("x-1").unwrap(),
            HostAgentIncarnation::new("run-7").unwrap(),
        );
        let first = HostDeliveryId::derive(
            &ceremony,
            HostDeliveryItemKind::Intervention,
            "i-1",
            &target.target_key(),
        )
        .unwrap();
        let second = HostDeliveryId::derive(
            &ceremony,
            HostDeliveryItemKind::Intervention,
            "i-1",
            &target.target_key(),
        )
        .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.as_str(), "c-1:intervention:i-1:agent:x-1:run-7");
    }

    #[test]
    fn serde_reuses_constructor_validation() {
        assert!(serde_json::from_str::<HostDeliveryId>("\"\"").is_err());
        let long = format!("\"{}\"", "x".repeat(513));
        assert!(serde_json::from_str::<HostDeliveryId>(&long).is_err());
    }
}
