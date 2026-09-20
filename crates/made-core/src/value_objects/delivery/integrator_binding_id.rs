use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::DomainError;

const MAX_LEN: usize = 256;

/// Identity of one integrator's binding to a ceremony or a system execution.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct IntegratorBindingId(String);

impl IntegratorBindingId {
    /// Construct a validated value.
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let raw = raw.into();
        let value = raw.trim();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "integrator_binding_id",
            });
        }
        if value.len() > MAX_LEN {
            return Err(DomainError::FieldTooLong {
                field: "integrator_binding_id",
                actual: value.len(),
                max: MAX_LEN,
            });
        }
        if value
            .chars()
            .any(|character| character.is_control() && character != '\n' && character != '\t')
        {
            return Err(DomainError::InvalidCharacters {
                field: "integrator_binding_id",
            });
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for IntegratorBindingId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

// Deserialisation goes through the constructor: a stored or host-supplied
// value must satisfy the same invariants as one built in process.
impl<'de> Deserialize<'de> for IntegratorBindingId {
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
        assert!(IntegratorBindingId::new("  ").is_err());
        assert!(IntegratorBindingId::new("x".repeat(MAX_LEN + 1)).is_err());
        assert_eq!(
            IntegratorBindingId::new("  value  ").unwrap().as_str(),
            "value"
        );
    }

    #[test]
    fn serde_reuses_constructor_validation() {
        assert!(serde_json::from_str::<IntegratorBindingId>("\"\"").is_err());
        assert_eq!(
            serde_json::from_str::<IntegratorBindingId>("\"value\"")
                .unwrap()
                .as_str(),
            "value"
        );
    }
}
