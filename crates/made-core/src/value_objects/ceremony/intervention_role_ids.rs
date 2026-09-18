use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

use super::RoleId;

/// Nonempty scoped recipients. Empty boundary lists mean the whole table instead.
/// Historical deserialization remains tolerant so journal replay is unchanged;
/// constructors and aggregate command validation enforce the current limits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct InterventionRoleIds(BTreeSet<RoleId>);

impl InterventionRoleIds {
    pub const MAX_ITEMS: usize = 100;
    const FIELD: &'static str = "ceremony_intervention.target_role_ids";

    pub fn new(role_ids: impl IntoIterator<Item = RoleId>) -> Result<Self, DomainError> {
        let mut roles = BTreeSet::new();
        for role in role_ids {
            let role = RoleId::new(role.as_str())?;
            if !roles.insert(role) {
                return Err(DomainError::InvalidDocument {
                    reason: format!("collection `{}` contains duplicate roles", Self::FIELD),
                });
            }
            if roles.len() > Self::MAX_ITEMS {
                return Err(DomainError::OutOfRange {
                    field: Self::FIELD,
                    value: roles.len() as f64,
                    min: 1.0,
                    max: Self::MAX_ITEMS as f64,
                });
            }
        }
        if roles.is_empty() {
            return Err(DomainError::EmptyCollection { field: Self::FIELD });
        }
        Ok(Self(roles))
    }

    #[must_use]
    pub fn as_set(&self) -> &BTreeSet<RoleId> {
        &self.0
    }

    pub(crate) fn validate(&self) -> Result<(), DomainError> {
        Self::new(self.0.iter().cloned()).map(|_| ())
    }
}

// Stored targets were already sets: every historical writer emitted sorted,
// distinct values. Keep historical cardinalities readable, but never erase
// duplicate input before a command has had a chance to refuse it.
impl<'de> Deserialize<'de> for InterventionRoleIds {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = Vec::<RoleId>::deserialize(deserializer)?;
        let mut roles = BTreeSet::new();
        for role in raw {
            if !roles.insert(role) {
                return Err(serde::de::Error::custom("duplicate intervention role"));
            }
        }
        Ok(Self(roles))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoped_recipients_are_nonempty_bounded_and_never_silently_deduplicated() {
        assert!(InterventionRoleIds::new([]).is_err());
        let roles = (0..=InterventionRoleIds::MAX_ITEMS)
            .map(|i| RoleId::new(format!("ROLE-{i}")).unwrap())
            .collect::<Vec<_>>();
        assert!(InterventionRoleIds::new(roles.clone()).is_err());
        assert!(InterventionRoleIds::new([roles[0].clone(), roles[0].clone()]).is_err());
        assert_eq!(
            InterventionRoleIds::new(roles[1..].to_vec())
                .unwrap()
                .as_set()
                .len(),
            100
        );
    }

    #[test]
    fn deserialization_never_erases_duplicates_before_validation() {
        assert!(
            serde_json::from_value::<InterventionRoleIds>(serde_json::json!(["ROLE", "ROLE"]))
                .is_err()
        );
        let restored: InterventionRoleIds =
            serde_json::from_value(serde_json::json!(["ROLE", " ROLE "])).unwrap();
        assert!(restored.validate().is_err());
    }

    #[test]
    fn historical_roles_keep_their_wire_shape_without_becoming_valid_new_targets() {
        for roles in [
            Vec::new(),
            (0..101).map(|i| format!("ROLE-{i:03}")).collect(),
        ] {
            let wire = serde_json::to_value(roles).unwrap();
            let restored: InterventionRoleIds = serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(serde_json::to_value(&restored).unwrap(), wire);
            assert!(restored.validate().is_err());
        }
    }
}
