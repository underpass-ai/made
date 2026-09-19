use serde::{Deserialize, Serialize};

use crate::error::DomainError;

/// Opaque keyset cursor for scanning execution operation roots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ExecutionRecoveryCursor(String);

impl ExecutionRecoveryCursor {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let value = raw.into();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "execution_recovery_cursor",
            });
        }
        if value.len() > 256 {
            return Err(DomainError::FieldTooLong {
                field: "execution_recovery_cursor",
                actual: value.len(),
                max: 256,
            });
        }
        if value.chars().any(char::is_control) {
            return Err(DomainError::InvalidCharacters {
                field: "execution_recovery_cursor",
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ExecutionRecoveryCursor {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<ExecutionRecoveryCursor> for String {
    fn from(value: ExecutionRecoveryCursor) -> Self {
        value.0
    }
}
