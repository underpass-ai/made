use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

use super::{ContextKey, RoleId};

/// A role selected from ceremony context at claim time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicRoleBinding {
    context_key: ContextKey,
    allowed_roles: BTreeSet<RoleId>,
}

impl DynamicRoleBinding {
    pub fn new(
        context_key: ContextKey,
        allowed_roles: impl IntoIterator<Item = RoleId>,
    ) -> Result<Self, DomainError> {
        let supplied = allowed_roles.into_iter().collect::<Vec<_>>();
        if supplied.is_empty() {
            return Err(DomainError::EmptyCollection {
                field: "dynamic_role_binding.allowed_roles",
            });
        }
        let canonical = supplied.iter().cloned().collect::<BTreeSet<_>>();
        if canonical.len() != supplied.len() {
            return Err(DomainError::AlreadyExists {
                what: "dynamic_role_binding.allowed_role",
            });
        }
        Ok(Self {
            context_key,
            allowed_roles: canonical,
        })
    }

    #[must_use]
    pub fn context_key(&self) -> &ContextKey {
        &self.context_key
    }

    #[must_use]
    pub fn allowed_roles(&self) -> &BTreeSet<RoleId> {
        &self.allowed_roles
    }

    #[must_use]
    pub fn allows(&self, role_id: &RoleId) -> bool {
        self.allowed_roles.contains(role_id)
    }
}
