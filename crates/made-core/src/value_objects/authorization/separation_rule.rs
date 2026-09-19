use serde::{Deserialize, Serialize};

use super::AuthorizationAction;
use crate::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeparationRule {
    approval_action: AuthorizationAction,
    execution_action: AuthorizationAction,
}

impl SeparationRule {
    pub fn new(
        approval_action: AuthorizationAction,
        execution_action: AuthorizationAction,
    ) -> Result<Self, DomainError> {
        if approval_action == execution_action {
            return Err(DomainError::InvariantViolated {
                reason: "authorization separation actions must differ",
            });
        }
        Ok(Self {
            approval_action,
            execution_action,
        })
    }

    #[must_use]
    pub const fn approval_action(self) -> AuthorizationAction {
        self.approval_action
    }

    #[must_use]
    pub const fn execution_action(self) -> AuthorizationAction {
        self.execution_action
    }
}
