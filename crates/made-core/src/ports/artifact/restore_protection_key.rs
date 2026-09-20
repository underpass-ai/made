use serde::{Deserialize, Serialize};

use crate::DomainError;

use super::ArtifactIdempotencyKey;

const RESTORE_PREFIX: &str = "restore:";

/// Typed identity for the temporary protection owned by one restore plan.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RestoreProtectionKey(ArtifactIdempotencyKey);

impl RestoreProtectionKey {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        let digest = value
            .strip_prefix(RESTORE_PREFIX)
            .ok_or(DomainError::InvalidCharacters {
                field: "restore_protection_key",
            })?;
        if digest.trim().is_empty() {
            return Err(DomainError::EmptyField {
                field: "restore_protection_key",
            });
        }
        Ok(Self(ArtifactIdempotencyKey::new(value)?))
    }

    pub fn for_plan_digest(plan_digest: &str) -> Result<Self, DomainError> {
        Self::new(format!("{RESTORE_PREFIX}{plan_digest}"))
    }

    #[must_use]
    pub fn as_idempotency_key(&self) -> &ArtifactIdempotencyKey {
        &self.0
    }

    #[must_use]
    pub fn into_idempotency_key(self) -> ArtifactIdempotencyKey {
        self.0
    }
}

impl TryFrom<String> for RestoreProtectionKey {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<ArtifactIdempotencyKey> for RestoreProtectionKey {
    type Error = DomainError;

    fn try_from(value: ArtifactIdempotencyKey) -> Result<Self, Self::Error> {
        Self::new(value.as_str())
    }
}

impl From<RestoreProtectionKey> for String {
    fn from(value: RestoreProtectionKey) -> Self {
        value.0.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_the_restore_namespace() {
        let key = RestoreProtectionKey::for_plan_digest("abc123").unwrap();
        assert_eq!(key.as_idempotency_key().as_str(), "restore:abc123");
        assert!(RestoreProtectionKey::new("receipt:abc123").is_err());
        assert!(RestoreProtectionKey::new("restore:").is_err());
    }
}
