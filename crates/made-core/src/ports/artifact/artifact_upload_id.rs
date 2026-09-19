use std::fmt;

use serde::{Deserialize, Serialize};

use crate::DomainError;

/// Opaque durable identity for one upload session.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ArtifactUploadId(String);

impl ArtifactUploadId {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "artifact_upload_id",
            });
        }
        if value.len() > 128 {
            return Err(DomainError::FieldTooLong {
                field: "artifact_upload_id",
                actual: value.len(),
                max: 128,
            });
        }
        if !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(DomainError::InvalidCharacters {
                field: "artifact_upload_id",
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ArtifactUploadId {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<ArtifactUploadId> for String {
    fn from(value: ArtifactUploadId) -> Self {
        value.0
    }
}

impl fmt::Display for ArtifactUploadId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
