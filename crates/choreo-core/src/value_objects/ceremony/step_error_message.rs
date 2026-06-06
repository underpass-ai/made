use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

const MAX_ERROR_MESSAGE_LEN: usize = 2048;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StepErrorMessage(String);

impl StepErrorMessage {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let value = raw.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(DomainError::EmptyField {
                field: "step_error_message",
            });
        }
        if trimmed.len() > MAX_ERROR_MESSAGE_LEN {
            return Err(DomainError::FieldTooLong {
                field: "step_error_message",
                actual: trimmed.len(),
                max: MAX_ERROR_MESSAGE_LEN,
            });
        }
        if trimmed.chars().any(char::is_control) {
            return Err(DomainError::InvalidCharacters {
                field: "step_error_message",
            });
        }
        Ok(Self(trimmed.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StepErrorMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
