use serde::{Deserialize, Serialize};

use crate::DomainError;

/// Named retention policy that caused a tombstone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ArtifactRetentionPolicy(String);

impl ArtifactRetentionPolicy {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DomainError::EmptyField {
                field: "artifact_retention_policy",
            });
        }
        if value.len() > 256 {
            return Err(DomainError::FieldTooLong {
                field: "artifact_retention_policy",
                actual: value.len(),
                max: 256,
            });
        }
        if value.chars().any(char::is_control) {
            return Err(DomainError::InvalidCharacters {
                field: "artifact_retention_policy",
            });
        }
        Ok(Self(value))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ArtifactRetentionPolicy {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<ArtifactRetentionPolicy> for String {
    fn from(value: ArtifactRetentionPolicy) -> Self {
        value.0
    }
}
