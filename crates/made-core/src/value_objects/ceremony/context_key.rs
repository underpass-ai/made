use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

/// A top-level key in ceremony context.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContextKey(String);

impl ContextKey {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let value = raw.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(DomainError::EmptyField {
                field: "context_key",
            });
        }
        if trimmed.chars().any(char::is_control) {
            return Err(DomainError::InvalidCharacters {
                field: "context_key",
            });
        }
        Ok(Self(trimmed.to_owned()))
    }

    pub fn from_role_from(raw: &str) -> Result<Self, DomainError> {
        let Some(key) = raw.strip_prefix("context.") else {
            return Err(DomainError::InvariantViolated {
                reason: "dynamic role binding must use context.<key>",
            });
        };
        Self::new(key)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn role_from(&self) -> String {
        format!("context.{}", self.0)
    }
}

impl fmt::Display for ContextKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
