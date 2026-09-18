use serde::{Deserialize, Serialize};

use crate::error::DomainError;

const MAX_LENGTH: usize = 1024;

/// Opaque reference to the source of imported artifact metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ArtifactImportRef(String);

impl ArtifactImportRef {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let raw = raw.into();
        let value = raw.trim();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "artifact_import_ref",
            });
        }
        if value.len() > MAX_LENGTH {
            return Err(DomainError::FieldTooLong {
                field: "artifact_import_ref",
                actual: value.len(),
                max: MAX_LENGTH,
            });
        }
        if value.chars().any(char::is_control) {
            return Err(DomainError::InvalidCharacters {
                field: "artifact_import_ref",
            });
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ArtifactImportRef {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<ArtifactImportRef> for String {
    fn from(value: ArtifactImportRef) -> Self {
        value.0
    }
}
