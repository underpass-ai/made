use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::ExecutionOperationId;
use crate::error::DomainError;

const SCHEME: &[u8] = b"made.execution-receipt.v1\0";

/// Deterministic identity of the sole terminal receipt for an operation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ExecutionReceiptId(String);

impl ExecutionReceiptId {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(DomainError::InvalidCharacters {
                field: "execution_receipt_id",
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn for_operation(operation_id: &ExecutionOperationId) -> Self {
        let mut digest = Sha256::new();
        digest.update(SCHEME);
        let bytes = operation_id.as_str().as_bytes();
        digest.update((bytes.len() as u64).to_be_bytes());
        digest.update(bytes);
        Self(format!("{:x}", digest.finalize()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ExecutionReceiptId {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<ExecutionReceiptId> for String {
    fn from(value: ExecutionReceiptId) -> Self {
        value.0
    }
}

impl fmt::Display for ExecutionReceiptId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
