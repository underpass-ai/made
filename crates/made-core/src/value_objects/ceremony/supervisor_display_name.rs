use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::DomainError;

const MAX_LEN: usize = 128;

/// How a supervisor is named to the agent it interrupted.
///
/// A label for a person or a service, kept short and free of control
/// characters because it is rendered straight into a host's transcript.
/// It is never an identity: the principal id is what authorization is
/// checked against, and this is only what the reader sees.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct SupervisorDisplayName(String);

impl SupervisorDisplayName {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let raw = raw.into();
        let value = raw.trim();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "supervisor_display_name",
            });
        }
        if value.len() > MAX_LEN {
            return Err(DomainError::FieldTooLong {
                field: "supervisor_display_name",
                actual: value.len(),
                max: MAX_LEN,
            });
        }
        if value.chars().any(char::is_control) {
            return Err(DomainError::InvalidCharacters {
                field: "supervisor_display_name",
            });
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SupervisorDisplayName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

// Deserialisation goes through the constructor so a stored or
// host-supplied name cannot reconstitute one the constructor refused.
impl<'de> Deserialize<'de> for SupervisorDisplayName {
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
    fn empty_oversized_and_control_names_are_refused() {
        assert!(SupervisorDisplayName::new("  ").is_err());
        assert!(SupervisorDisplayName::new("x".repeat(MAX_LEN + 1)).is_err());
        assert!(SupervisorDisplayName::new("line\nbreak").is_err());
        assert_eq!(
            SupervisorDisplayName::new("  Release manager  ")
                .unwrap()
                .as_str(),
            "Release manager"
        );
    }

    #[test]
    fn serde_reuses_constructor_validation() {
        assert!(serde_json::from_str::<SupervisorDisplayName>("\"\"").is_err());
    }
}
