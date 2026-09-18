use serde::{Deserialize, Serialize};

use super::ExecutionRequestDigest;
use crate::error::DomainError;

const MAX_BYTES: usize = 1_048_576;

/// Canonical semantic input sealed before an external operation starts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ExecutionRequestBytes(Vec<u8>);

impl ExecutionRequestBytes {
    pub fn new(bytes: Vec<u8>) -> Result<Self, DomainError> {
        if bytes.is_empty() {
            return Err(DomainError::EmptyField {
                field: "execution_request_bytes",
            });
        }
        if bytes.len() > MAX_BYTES {
            return Err(DomainError::FieldTooLong {
                field: "execution_request_bytes",
                actual: bytes.len(),
                max: MAX_BYTES,
            });
        }
        Ok(Self(bytes))
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    #[must_use]
    pub fn digest(&self) -> ExecutionRequestDigest {
        ExecutionRequestDigest::for_bytes(&self.0)
    }
}
