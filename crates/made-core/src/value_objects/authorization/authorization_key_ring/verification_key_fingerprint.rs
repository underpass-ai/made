use serde::{Deserialize, Serialize};

use crate::DomainError;

/// Fingerprint of a key, not key material. It is safe to persist in audit
/// evidence and to compare between replicas.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct VerificationKeyFingerprint(String);

impl VerificationKeyFingerprint {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(DomainError::InvariantViolated {
                reason: "verification key fingerprint must be lowercase sha256",
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for VerificationKeyFingerprint {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<VerificationKeyFingerprint> for String {
    fn from(value: VerificationKeyFingerprint) -> Self {
        value.0
    }
}
