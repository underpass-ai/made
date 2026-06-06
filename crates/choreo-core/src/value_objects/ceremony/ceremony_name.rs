use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

const MAX_NAME_LEN: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CeremonyName(String);

impl CeremonyName {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let value = raw.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(DomainError::EmptyField {
                field: "ceremony_name",
            });
        }
        if trimmed.len() > MAX_NAME_LEN {
            return Err(DomainError::FieldTooLong {
                field: "ceremony_name",
                actual: trimmed.len(),
                max: MAX_NAME_LEN,
            });
        }
        if trimmed
            .chars()
            .any(|ch| !(ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_'))
        {
            return Err(DomainError::InvalidCharacters {
                field: "ceremony_name",
            });
        }
        Ok(Self(trimmed.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CeremonyName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
