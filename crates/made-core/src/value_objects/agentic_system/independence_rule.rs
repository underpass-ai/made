use serde::{Deserialize, Serialize};

use crate::error::DomainError;

use super::SystemRoleId;

/// A demand that one role's work be judged by somebody other than
/// whoever did it.
///
/// The rule is about roles; whether it holds is about participants,
/// and the analysis is what compares the two. A design can therefore
/// state the requirement long before anybody knows who will play it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct IndependenceRule {
    reviewer: SystemRoleId,
    reviewed: SystemRoleId,
}

impl IndependenceRule {
    /// Construct a rule between two distinct roles.
    ///
    /// A role required to be independent of itself is not a strict
    /// rule; it is one that can never hold, and refusing it at
    /// construction keeps an unsatisfiable design from being written
    /// down at all.
    pub fn new(reviewer: SystemRoleId, subject: SystemRoleId) -> Result<Self, DomainError> {
        if reviewer == subject {
            return Err(DomainError::InvariantViolated {
                reason: "a role cannot be required to be independent of itself",
            });
        }
        Ok(Self {
            reviewer,
            reviewed: subject,
        })
    }

    #[must_use]
    pub const fn reviewer(&self) -> &SystemRoleId {
        &self.reviewer
    }

    #[must_use]
    pub const fn reviewed(&self) -> &SystemRoleId {
        &self.reviewed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_role_cannot_review_itself_independently() {
        let reviewer = SystemRoleId::new("reviewer").unwrap();

        assert!(IndependenceRule::new(reviewer.clone(), reviewer).is_err());
        assert!(IndependenceRule::new(
            SystemRoleId::new("reviewer").unwrap(),
            SystemRoleId::new("author").unwrap()
        )
        .is_ok());
    }
}
