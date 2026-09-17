use serde::{Deserialize, Serialize};

use crate::error::DomainError;

const MAX_STEP_INSTRUCTIONS: usize = 16_384;

/// The instruction text a ceremony stage gives its handler.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StepInstructions(String);

impl StepInstructions {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(DomainError::EmptyField {
                field: "step_instructions",
            });
        }
        if trimmed.chars().count() > MAX_STEP_INSTRUCTIONS {
            return Err(DomainError::FieldTooLong {
                field: "step_instructions",
                max: MAX_STEP_INSTRUCTIONS,
                actual: trimmed.chars().count(),
            });
        }
        Ok(Self(trimmed.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
