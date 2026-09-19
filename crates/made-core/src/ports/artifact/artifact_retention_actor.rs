use serde::{Deserialize, Serialize};

use crate::DomainError;

/// Host-asserted actor recorded for retention audit; C5.7 will authorize it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ArtifactRetentionActor(String);

impl ArtifactRetentionActor {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DomainError::EmptyField {
                field: "artifact_retention_actor",
            });
        }
        if value.len() > 256 {
            return Err(DomainError::FieldTooLong {
                field: "artifact_retention_actor",
                actual: value.len(),
                max: 256,
            });
        }
        if value.chars().any(char::is_control) {
            return Err(DomainError::InvalidCharacters {
                field: "artifact_retention_actor",
            });
        }
        Ok(Self(value))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ArtifactRetentionActor {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<ArtifactRetentionActor> for String {
    fn from(value: ArtifactRetentionActor) -> Self {
        value.0
    }
}
