use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

/// Which edition of one agentic system design this is.
///
/// Its own type rather than `CeremonyRevision`: a design revision and a
/// session revision count different things, and sharing the newtype
/// would let one be passed where the other belongs and compare equal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgenticSystemRevision(u64);

impl AgenticSystemRevision {
    /// The revision a freshly created design carries.
    pub const INITIAL: Self = Self(1);

    pub fn new(value: u64) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "agentic_system_revision",
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// The revision an edit of this one produces.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

impl fmt::Display for AgenticSystemRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_design_starts_at_one_and_counts_up() {
        assert_eq!(AgenticSystemRevision::INITIAL.get(), 1);
        assert_eq!(AgenticSystemRevision::INITIAL.next().get(), 2);
        assert!(AgenticSystemRevision::new(0).is_err());
        assert_eq!(AgenticSystemRevision::new(7).unwrap().to_string(), "7");
    }
}
