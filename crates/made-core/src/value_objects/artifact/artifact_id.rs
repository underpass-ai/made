use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

const MAX_LENGTH: usize = 256;

/// Stable opaque identity for artifact metadata, never a path or URL.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ArtifactId(String);

impl ArtifactId {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let raw = raw.into();
        let value = raw.trim();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "artifact_id",
            });
        }
        if value.len() > MAX_LENGTH {
            return Err(DomainError::FieldTooLong {
                field: "artifact_id",
                actual: value.len(),
                max: MAX_LENGTH,
            });
        }
        if value.chars().any(char::is_control) {
            return Err(DomainError::InvalidCharacters {
                field: "artifact_id",
            });
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ArtifactId {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<ArtifactId> for String {
    fn from(value: ArtifactId) -> Self {
        value.0
    }
}

impl fmt::Display for ArtifactId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
