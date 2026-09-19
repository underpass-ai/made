use serde::{Deserialize, Serialize};

use crate::error::DomainError;

const MAX_PREFIX_LEN: usize = 256;

/// A literal, case-sensitive prefix for ceremony identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CeremonyIdPrefix(String);

impl CeremonyIdPrefix {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let value = raw.into();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "ceremony_id_prefix",
            });
        }
        if value.len() > MAX_PREFIX_LEN {
            return Err(DomainError::FieldTooLong {
                field: "ceremony_id_prefix",
                actual: value.len(),
                max: MAX_PREFIX_LEN,
            });
        }
        if value.chars().any(char::is_control) {
            return Err(DomainError::InvalidCharacters {
                field: "ceremony_id_prefix",
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_is_literal_and_bounded() {
        assert_eq!(CeremonyIdPrefix::new("team-").unwrap().as_str(), "team-");
        assert!(CeremonyIdPrefix::new("").is_err());
        assert!(CeremonyIdPrefix::new("x".repeat(257)).is_err());
        assert!(CeremonyIdPrefix::new("team\n").is_err());
    }
}
