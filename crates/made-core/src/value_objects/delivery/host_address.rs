use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::DomainError;

const MAX_LEN: usize = 512;

/// Opaque address of a host destination: a session, a task, a queue.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct HostAddress(String);

impl HostAddress {
    /// Construct a validated value.
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let raw = raw.into();
        let value = raw.trim();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "host_address",
            });
        }
        if value.len() > MAX_LEN {
            return Err(DomainError::FieldTooLong {
                field: "host_address",
                actual: value.len(),
                max: MAX_LEN,
            });
        }
        if value
            .chars()
            .any(|character| character.is_control() && character != '\n' && character != '\t')
        {
            return Err(DomainError::InvalidCharacters {
                field: "host_address",
            });
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for HostAddress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

// Deserialisation goes through the constructor: a stored or host-supplied
// value must satisfy the same invariants as one built in process.
impl<'de> Deserialize<'de> for HostAddress {
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
        assert!(HostAddress::new("  ").is_err());
        assert!(HostAddress::new("x".repeat(MAX_LEN + 1)).is_err());
        assert_eq!(HostAddress::new("  value  ").unwrap().as_str(), "value");
    }

    #[test]
    fn serde_reuses_constructor_validation() {
        assert!(serde_json::from_str::<HostAddress>("\"\"").is_err());
        assert_eq!(
            serde_json::from_str::<HostAddress>("\"value\"")
                .unwrap()
                .as_str(),
            "value"
        );
    }
}
