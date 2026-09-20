use serde::{Deserialize, Deserializer, Serialize};

use crate::error::DomainError;

use super::SystemRoleId;

/// A demand that one role's work be judged by somebody other than
/// whoever did it.
///
/// The rule is about roles; whether it holds is about participants,
/// and the analysis is what compares the two. A design can therefore
/// state the requirement long before anybody knows who will play it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
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

/// Decoding goes through the constructor, so a rule that could never
/// hold cannot arrive from a document and sit in a design looking
/// satisfied.
impl<'de> Deserialize<'de> for IndependenceRule {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            reviewer: SystemRoleId,
            reviewed: SystemRoleId,
        }

        let wire = Wire::deserialize(deserializer)?;
        Self::new(wire.reviewer, wire.reviewed).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_self_review_is_refused_on_the_way_in() {
        assert!(
            serde_json::from_str::<IndependenceRule>(r#"{"reviewer":"a","reviewed":"a"}"#).is_err()
        );
        assert!(
            serde_json::from_str::<IndependenceRule>(r#"{"reviewer":"a","reviewed":"b"}"#).is_ok()
        );
    }

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
