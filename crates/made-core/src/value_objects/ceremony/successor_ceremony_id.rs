use sha2::{Digest, Sha256};

use super::{CeremonyId, IdempotencyKey};
use crate::error::DomainError;

/// The id of the successor a plan opens, derived from the plan itself.
///
/// Derived rather than supplied, for the same reason a child's id is:
/// the sealing of the plan and the opening of the stream are two
/// appends, and a crash between them has to be resumable. A caller
/// that chose the id could retry with a different one and open a
/// second successor for one plan; a derived id makes the retry land on
/// the stream the first attempt was opening.
///
/// The predecessor is spelled out rather than hashed in, so the
/// relation is legible in a store listing without reading a stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuccessorCeremonyId(CeremonyId);

impl SuccessorCeremonyId {
    pub fn derive(
        predecessor: &CeremonyId,
        plan_id: &IdempotencyKey,
    ) -> Result<Self, DomainError> {
        let mut digest = Sha256::new();
        digest.update(b"made.ceremony-successor.v1\0");
        for part in [predecessor.as_str(), plan_id.as_str()] {
            digest.update((part.len() as u64).to_be_bytes());
            digest.update(part.as_bytes());
        }
        let digest = digest.finalize();
        let mut suffix = String::with_capacity(16);
        for byte in &digest[..8] {
            suffix.push_str(&format!("{byte:02x}"));
        }
        CeremonyId::new(format!("{}.s.{suffix}", predecessor.as_str())).map(Self)
    }

    #[must_use]
    pub const fn as_ceremony_id(&self) -> &CeremonyId {
        &self.0
    }

    #[must_use]
    pub fn into_ceremony_id(self) -> CeremonyId {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(raw: &str) -> IdempotencyKey {
        IdempotencyKey::new(raw).unwrap()
    }

    #[test]
    fn one_plan_names_one_successor_however_often_it_is_retried() {
        let predecessor = CeremonyId::new("review-42").unwrap();
        let first = SuccessorCeremonyId::derive(&predecessor, &plan("plan-1")).unwrap();
        let again = SuccessorCeremonyId::derive(&predecessor, &plan("plan-1")).unwrap();
        let other = SuccessorCeremonyId::derive(&predecessor, &plan("plan-2")).unwrap();

        assert_eq!(first, again);
        assert_ne!(first, other);
        assert!(first
            .as_ceremony_id()
            .as_str()
            .starts_with("review-42.s."));
        assert_eq!(first.as_ceremony_id().as_str().len(), "review-42.s.".len() + 16);
    }
}
