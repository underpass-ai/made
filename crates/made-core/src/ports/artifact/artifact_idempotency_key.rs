use serde::{Deserialize, Serialize};

use crate::DomainError;

/// Caller-selected key that makes beginning an upload replay-safe.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ArtifactIdempotencyKey(String);

impl ArtifactIdempotencyKey {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DomainError::EmptyField {
                field: "artifact_idempotency_key",
            });
        }
        if value.len() > 256 {
            return Err(DomainError::FieldTooLong {
                field: "artifact_idempotency_key",
                actual: value.len(),
                max: 256,
            });
        }
        if value.chars().any(char::is_control) {
            return Err(DomainError::InvalidCharacters {
                field: "artifact_idempotency_key",
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ArtifactIdempotencyKey {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<ArtifactIdempotencyKey> for String {
    fn from(value: ArtifactIdempotencyKey) -> Self {
        value.0
    }
}
