use serde::{Deserialize, Serialize};

use crate::error::DomainError;
use crate::value_objects::AuditActorId;

use super::{RoleId, SupervisorDisplayName};

/// Somebody who may ask a question without holding a seat at the table.
///
/// A supervisor route exists because the person watching a ceremony is
/// often not playing in it, and making them take a role in order to ask
/// would hand them every action that role has. Authority to ask is not
/// authority to mutate: this type carries the asker's identity and
/// nothing else, and closing the item it opens stays with the item's
/// requester, which is the synthetic role derived from this principal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupervisorPrincipal {
    principal_id: AuditActorId,
    display: SupervisorDisplayName,
}

impl SupervisorPrincipal {
    #[must_use]
    pub const fn new(principal_id: AuditActorId, display: SupervisorDisplayName) -> Self {
        Self {
            principal_id,
            display,
        }
    }

    #[must_use]
    pub const fn principal_id(&self) -> &AuditActorId {
        &self.principal_id
    }

    #[must_use]
    pub const fn display(&self) -> &SupervisorDisplayName {
        &self.display
    }

    /// The seat this principal asks under.
    ///
    /// Derived rather than chosen, so a supervisor can never spell a
    /// role the definition declares and inherit what that role may do.
    /// The prefix is what tells a reader of the journal that the asker
    /// was outside the table.
    pub fn requesting_role(&self) -> Result<RoleId, DomainError> {
        RoleId::new(format!("supervisor:{}", self.principal_id.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn principal(id: &str) -> SupervisorPrincipal {
        SupervisorPrincipal::new(
            AuditActorId::new(id),
            SupervisorDisplayName::new("Release manager").unwrap(),
        )
    }

    #[test]
    fn the_asking_seat_is_derived_and_cannot_collide_with_a_declared_role() {
        let role = principal("alice").requesting_role().unwrap();
        assert_eq!(role.as_str(), "supervisor:alice");
        assert_ne!(role.as_str(), "ENGINEER");
    }
}
