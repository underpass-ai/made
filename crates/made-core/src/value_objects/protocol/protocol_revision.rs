use serde::{Deserialize, Serialize};

use crate::error::DomainError;

/// How many times a protocol instance has been committed.
///
/// The revision lives in the storage contract rather than in the
/// aggregate: it protects a write against a concurrent write, not any
/// invariant of the protocol itself. Keeping it out of
/// `ProtocolInstance` also keeps the shape a host has already persisted
/// from changing underneath it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProtocolRevision(u64);

impl ProtocolRevision {
    /// The revision a protocol reaches on its first successful commit.
    pub const INITIAL: Self = Self(1);

    pub fn new(value: u64) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "protocol_revision",
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn value(self) -> u64 {
        self.0
    }

    #[must_use]
    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_protocol_is_at_least_at_the_initial_revision() {
        assert_eq!(ProtocolRevision::INITIAL.value(), 1);
        assert!(ProtocolRevision::new(0).is_err());
    }

    #[test]
    fn revisions_advance_by_one() {
        assert_eq!(ProtocolRevision::INITIAL.next().value(), 2);
    }
}
