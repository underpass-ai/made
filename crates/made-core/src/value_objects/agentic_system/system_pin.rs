use std::fmt;

use serde::{Deserialize, Serialize};

use crate::value_objects::AgenticSystemId;

use super::{AgenticSystemDigest, AgenticSystemRevision};

/// An immutable reference from a run to the design it is running.
///
/// The same reasoning as a ceremony's definition pin, one level up:
/// identity and revision say which design, and the digest says which
/// bytes, so a run can be shown afterwards to have composed exactly
/// what it claims.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SystemPin {
    id: AgenticSystemId,
    revision: AgenticSystemRevision,
    digest: AgenticSystemDigest,
}

impl SystemPin {
    #[must_use]
    pub const fn new(
        id: AgenticSystemId,
        revision: AgenticSystemRevision,
        digest: AgenticSystemDigest,
    ) -> Self {
        Self {
            id,
            revision,
            digest,
        }
    }

    #[must_use]
    pub const fn id(&self) -> &AgenticSystemId {
        &self.id
    }

    #[must_use]
    pub const fn revision(&self) -> AgenticSystemRevision {
        self.revision
    }

    #[must_use]
    pub const fn digest(&self) -> AgenticSystemDigest {
        self.digest
    }

    /// Whether this pin names the same design bytes as another.
    #[must_use]
    pub fn matches(&self, other: &Self) -> bool {
        self == other
    }
}

impl fmt::Display for SystemPin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}@{}", self.id, self.revision)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pin(fill: u8) -> SystemPin {
        SystemPin::new(
            AgenticSystemId::new("delivery").unwrap(),
            AgenticSystemRevision::INITIAL,
            AgenticSystemDigest::from_bytes([fill; 32]),
        )
    }

    #[test]
    fn a_revision_republished_with_other_bytes_is_not_the_pin_that_ran() {
        assert!(pin(0x0a).matches(&pin(0x0a)));
        assert!(!pin(0x0a).matches(&pin(0x0b)));
        assert_eq!(pin(0x0a).to_string(), "delivery@1");
    }
}
