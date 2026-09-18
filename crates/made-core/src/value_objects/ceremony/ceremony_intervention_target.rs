use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

use super::{InterventionRoleIds, RoleId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "role_ids", rename_all = "snake_case")]
pub enum CeremonyInterventionTarget {
    Table,
    Roles(InterventionRoleIds),
}

impl CeremonyInterventionTarget {
    #[must_use]
    pub const fn table() -> Self {
        Self::Table
    }

    pub fn roles(role_ids: impl IntoIterator<Item = RoleId>) -> Result<Self, DomainError> {
        InterventionRoleIds::new(role_ids).map(Self::Roles)
    }

    pub(crate) fn validate(&self) -> Result<(), DomainError> {
        match self {
            Self::Table => Ok(()),
            Self::Roles(role_ids) => role_ids.validate(),
        }
    }

    #[must_use]
    pub fn accepts(&self, role_id: &RoleId) -> bool {
        match self {
            Self::Table => true,
            Self::Roles(role_ids) => role_ids.as_set().contains(role_id),
        }
    }

    #[must_use]
    pub fn role_ids(&self) -> Option<&BTreeSet<RoleId>> {
        match self {
            Self::Table => None,
            Self::Roles(role_ids) => Some(role_ids.as_set()),
        }
    }

    #[must_use]
    pub const fn is_table(&self) -> bool {
        matches!(self, Self::Table)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_accepts_every_role_and_scoped_target_does_not() {
        let engineer = RoleId::new("ENGINEER").unwrap();
        let observer = RoleId::new("OBSERVER").unwrap();

        assert!(CeremonyInterventionTarget::table().accepts(&engineer));
        let target = CeremonyInterventionTarget::roles([observer.clone()]).unwrap();
        assert!(target.accepts(&observer));
        assert!(!target.accepts(&engineer));
    }
}
