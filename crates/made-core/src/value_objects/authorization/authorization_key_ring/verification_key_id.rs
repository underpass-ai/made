use serde::{Deserialize, Serialize};

use crate::DomainError;

/// Public identifier for a verification key. Material never crosses this
/// boundary; the host keeps private key bytes in its secret store.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct VerificationKeyId(String);

impl VerificationKeyId {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.is_empty() || value.len() > 128 || !value.bytes().all(is_safe_key_byte) {
            return Err(DomainError::InvariantViolated {
                reason: "verification key id must be a bounded token",
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for VerificationKeyId {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<VerificationKeyId> for String {
    fn from(value: VerificationKeyId) -> Self {
        value.0
    }
}

fn is_safe_key_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')
}
