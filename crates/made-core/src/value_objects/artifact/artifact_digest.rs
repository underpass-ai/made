use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

const PREFIX: &str = "sha256:";
const HEX_LENGTH: usize = 64;

/// Canonical SHA-256 content digest, including its algorithm label.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ArtifactDigest(String);

impl ArtifactDigest {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let value = raw.into();
        let Some(hex) = value.strip_prefix(PREFIX) else {
            return Err(DomainError::InvalidCharacters {
                field: "artifact_digest",
            });
        };
        if hex.len() != HEX_LENGTH
            || !hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(DomainError::InvalidCharacters {
                field: "artifact_digest",
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ArtifactDigest {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<ArtifactDigest> for String {
    fn from(value: ArtifactDigest) -> Self {
        value.0
    }
}

impl fmt::Display for ArtifactDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
