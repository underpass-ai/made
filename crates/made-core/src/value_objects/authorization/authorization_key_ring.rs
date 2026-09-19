use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::DomainError;

mod cursor_transition;
mod verification_key;
mod verification_key_fingerprint;
mod verification_key_id;

pub use cursor_transition::CursorTransition;
pub use verification_key::VerificationKey;
pub use verification_key_fingerprint::VerificationKeyFingerprint;
pub use verification_key_id::VerificationKeyId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizationKeyRing {
    active: VerificationKey,
    verification: Vec<VerificationKey>,
    cursor_generation: u64,
}

impl AuthorizationKeyRing {
    pub fn new(active: VerificationKey) -> Result<Self, DomainError> {
        let ring = Self {
            active,
            verification: Vec::new(),
            cursor_generation: 0,
        };
        ring.validate()?;
        Ok(ring)
    }

    pub fn rotate(
        &self,
        next_active: VerificationKey,
        transition: CursorTransition,
    ) -> Result<Self, DomainError> {
        let mut verification = self.verification.clone();
        verification.push(self.active.clone());
        let ring = Self {
            active: next_active,
            verification,
            cursor_generation: self.cursor_generation
                + u64::from(matches!(transition, CursorTransition::Invalidate)),
        };
        ring.validate()?;
        Ok(ring)
    }

    pub fn validate(&self) -> Result<(), DomainError> {
        let mut ids = BTreeSet::new();
        let mut fingerprints = BTreeSet::new();
        let all = std::iter::once(&self.active).chain(self.verification.iter());
        for key in all {
            if !ids.insert(key.id().clone()) || !fingerprints.insert(key.fingerprint().clone()) {
                return Err(DomainError::InvariantViolated {
                    reason: "verification key ring contains a duplicate key",
                });
            }
        }
        Ok(())
    }

    #[must_use]
    pub const fn active(&self) -> &VerificationKey {
        &self.active
    }

    #[must_use]
    pub fn accepts(&self, id: &VerificationKeyId, at: OffsetDateTime) -> bool {
        std::iter::once(&self.active)
            .chain(self.verification.iter())
            .any(|key| key.id() == id && key.accepts_at(at))
    }

    #[must_use]
    pub const fn cursor_generation(&self) -> u64 {
        self.cursor_generation
    }

    #[must_use]
    pub fn verification_keys(&self) -> &[VerificationKey] {
        &self.verification
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(id: &str, byte: u8) -> VerificationKey {
        VerificationKey::new(
            VerificationKeyId::new(id).unwrap(),
            VerificationKeyFingerprint::new(char::from(byte).to_string().repeat(64)).unwrap(),
            OffsetDateTime::UNIX_EPOCH,
            None,
        )
        .unwrap()
    }

    #[test]
    fn rotation_keeps_old_key_only_for_the_declared_transition() {
        let ring = AuthorizationKeyRing::new(key("old", b'a')).unwrap();
        let next = ring
            .rotate(key("new", b'b'), CursorTransition::Invalidate)
            .unwrap();
        assert!(next.accepts(
            &VerificationKeyId::new("old").unwrap(),
            OffsetDateTime::UNIX_EPOCH
        ));
        assert_eq!(next.cursor_generation(), 1);
    }

    #[test]
    fn duplicate_identity_or_fingerprint_is_rejected() {
        let ring = AuthorizationKeyRing::new(key("old", b'a')).unwrap();
        assert!(ring
            .rotate(key("old", b'b'), CursorTransition::Preserve)
            .is_err());
        assert!(ring
            .rotate(key("new", b'a'), CursorTransition::Preserve)
            .is_err());
    }

    #[test]
    fn expired_key_is_not_accepted() {
        let expired = VerificationKey::new(
            VerificationKeyId::new("expired").unwrap(),
            VerificationKeyFingerprint::new("c".repeat(64)).unwrap(),
            OffsetDateTime::UNIX_EPOCH,
            Some(OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(1)),
        )
        .unwrap();
        let ring = AuthorizationKeyRing::new(expired).unwrap();
        assert!(!ring.accepts(
            &VerificationKeyId::new("expired").unwrap(),
            OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(1)
        ));
    }
}
