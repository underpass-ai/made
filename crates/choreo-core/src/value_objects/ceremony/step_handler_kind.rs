use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

const MAX_HANDLER_KIND_LEN: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StepHandlerKind(String);

impl StepHandlerKind {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let value = raw.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(DomainError::EmptyField {
                field: "step_handler_kind",
            });
        }
        if trimmed.len() > MAX_HANDLER_KIND_LEN {
            return Err(DomainError::FieldTooLong {
                field: "step_handler_kind",
                actual: trimmed.len(),
                max: MAX_HANDLER_KIND_LEN,
            });
        }
        if trimmed.chars().any(|ch| {
            !(ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '_' | '-' | '.'))
        }) {
            return Err(DomainError::InvalidCharacters {
                field: "step_handler_kind",
            });
        }
        Ok(Self(trimmed.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StepHandlerKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
