use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::DomainError;

const MAX_LEN: usize = 2000;

/// What a host said about a delivery it observed, in its own words.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct DeliveryNote(String);

impl DeliveryNote {
    /// Construct a validated value.
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let raw = raw.into();
        let value = raw.trim();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "delivery_note",
            });
        }
        if value.len() > MAX_LEN {
            return Err(DomainError::FieldTooLong {
                field: "delivery_note",
                actual: value.len(),
                max: MAX_LEN,
            });
        }
        if value
            .chars()
            .any(|character| character.is_control() && character != '\n' && character != '\t')
        {
            return Err(DomainError::InvalidCharacters {
                field: "delivery_note",
            });
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DeliveryNote {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

// Deserialisation goes through the constructor: a stored or host-supplied
// value must satisfy the same invariants as one built in process.
impl<'de> Deserialize<'de> for DeliveryNote {
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
    fn empty_and_oversized_values_are_refused() {
        assert!(DeliveryNote::new("  ").is_err());
        assert!(DeliveryNote::new("x".repeat(MAX_LEN + 1)).is_err());
        assert_eq!(DeliveryNote::new("  value  ").unwrap().as_str(), "value");
    }

    #[test]
    fn serde_reuses_constructor_validation() {
        assert!(serde_json::from_str::<DeliveryNote>("\"\"").is_err());
        assert_eq!(
            serde_json::from_str::<DeliveryNote>("\"value\"")
                .unwrap()
                .as_str(),
            "value"
        );
    }
}
