use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::DomainError;

const SCHEME: &[u8] = b"made.execution-request.v1\0";

/// Digest of the semantic request bytes sealed by the first intent.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ExecutionRequestDigest(String);

impl ExecutionRequestDigest {
    #[must_use]
    pub fn for_bytes(bytes: &[u8]) -> Self {
        let mut digest = Sha256::new();
        digest.update(SCHEME);
        digest.update((bytes.len() as u64).to_be_bytes());
        digest.update(bytes);
        Self(format!("{:x}", digest.finalize()))
    }

    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(DomainError::InvalidCharacters {
                field: "execution_request_digest",
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ExecutionRequestDigest {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<ExecutionRequestDigest> for String {
    fn from(value: ExecutionRequestDigest) -> Self {
        value.0
    }
}

impl fmt::Display for ExecutionRequestDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
