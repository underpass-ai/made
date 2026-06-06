use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

use super::{RoleAction, RoleId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyRole {
    id: RoleId,
    allowed_actions: BTreeSet<RoleAction>,
}

impl CeremonyRole {
    pub fn new(
        id: RoleId,
        allowed_actions: impl IntoIterator<Item = RoleAction>,
    ) -> Result<Self, DomainError> {
        let allowed_actions: BTreeSet<RoleAction> = allowed_actions.into_iter().collect();
        if allowed_actions.is_empty() {
            return Err(DomainError::EmptyCollection {
                field: "ceremony_role.allowed_actions",
            });
        }
        Ok(Self {
            id,
            allowed_actions,
        })
    }

    #[must_use]
    pub fn id(&self) -> &RoleId {
        &self.id
    }

    #[must_use]
    pub fn allowed_actions(&self) -> &BTreeSet<RoleAction> {
        &self.allowed_actions
    }

    #[must_use]
    pub fn allows(&self, action: &RoleAction) -> bool {
        self.allowed_actions.contains(action)
    }
}
