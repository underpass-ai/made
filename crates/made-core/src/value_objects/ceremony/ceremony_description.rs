use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

const MAX_DESCRIPTION_LEN: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CeremonyDescription(String);

impl CeremonyDescription {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let value = raw.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(DomainError::EmptyField {
                field: "ceremony_description",
            });
        }
        if trimmed.len() > MAX_DESCRIPTION_LEN {
            return Err(DomainError::FieldTooLong {
                field: "ceremony_description",
                actual: trimmed.len(),
                max: MAX_DESCRIPTION_LEN,
            });
        }
        if trimmed.chars().any(char::is_control) {
            return Err(DomainError::InvalidCharacters {
                field: "ceremony_description",
            });
        }
        Ok(Self(trimmed.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CeremonyDescription {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
