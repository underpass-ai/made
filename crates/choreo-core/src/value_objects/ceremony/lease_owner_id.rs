use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LeaseOwnerId(String);

impl LeaseOwnerId {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let value = raw.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(DomainError::EmptyField {
                field: "lease_owner_id",
            });
        }
        if trimmed.chars().any(char::is_control) {
            return Err(DomainError::InvalidCharacters {
                field: "lease_owner_id",
            });
        }
        Ok(Self(trimmed.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LeaseOwnerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
