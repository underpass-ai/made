use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

const MAX_LENGTH: usize = 255;

/// Canonical media type without parameters.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ArtifactMediaType(String);

impl ArtifactMediaType {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let raw = raw.into();
        let value = raw.trim();
        if value.is_empty() {
            return Err(DomainError::EmptyField {
                field: "artifact_media_type",
            });
        }
        if value.len() > MAX_LENGTH {
            return Err(DomainError::FieldTooLong {
                field: "artifact_media_type",
                actual: value.len(),
                max: MAX_LENGTH,
            });
        }
        let mut parts = value.split('/');
        let (Some(top), Some(sub), None) = (parts.next(), parts.next(), parts.next()) else {
            return Err(DomainError::InvalidCharacters {
                field: "artifact_media_type",
            });
        };
        if !valid_token(top) || !valid_token(sub) {
            return Err(DomainError::InvalidCharacters {
                field: "artifact_media_type",
            });
        }
        Ok(Self(value.to_ascii_lowercase()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn valid_token(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'!' | b'#' | b'$' | b'&' | b'^' | b'_' | b'.' | b'+' | b'-'
                )
        })
}

impl TryFrom<String> for ArtifactMediaType {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<ArtifactMediaType> for String {
    fn from(value: ArtifactMediaType) -> Self {
        value.0
    }
}

impl fmt::Display for ArtifactMediaType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
