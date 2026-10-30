use serde::{Deserialize, Serialize};

use crate::error::DomainError;

const MAX_PREFIX_LEN: usize = 256;

/// A literal, case-sensitive prefix for protocol identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProtocolIdPrefix(String);

impl ProtocolIdPrefix {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let value = raw.into();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "protocol_id_prefix",
            });
        }
        if value.len() > MAX_PREFIX_LEN {
            return Err(DomainError::FieldTooLong {
                field: "protocol_id_prefix",
                actual: value.len(),
                max: MAX_PREFIX_LEN,
            });
        }
        if value.chars().any(char::is_control) {
            return Err(DomainError::InvalidCharacters {
                field: "protocol_id_prefix",
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
        assert_eq!(ProtocolIdPrefix::new("team-").unwrap().as_str(), "team-");
        assert!(ProtocolIdPrefix::new("").is_err());
        assert!(ProtocolIdPrefix::new("x".repeat(257)).is_err());
        assert!(ProtocolIdPrefix::new("team\n").is_err());
    }
}
