use std::collections::BTreeMap;

use made_app::workers::{
    CeremonyWorkerCapacity, CeremonyWorkerCost, CeremonyWorkerPriority, CeremonyWorkerRootPolicy,
    CeremonyWorkerRootPolicyPort, CeremonyWorkerWeight,
};
use made_core::value_objects::CeremonyId;
use made_core::DomainError;

#[derive(Debug)]
pub struct ConfiguredWorkerRootPolicy {
    default: CeremonyWorkerRootPolicy,
    roots: BTreeMap<CeremonyId, CeremonyWorkerRootPolicy>,
}

impl ConfiguredWorkerRootPolicy {
    pub fn from_json(
        default: CeremonyWorkerRootPolicy,
        document: Option<&str>,
    ) -> Result<Self, DomainError> {
        let Some(document) = document else {
            return Ok(Self {
                default,
                roots: BTreeMap::new(),
            });
        };
        let object = serde_json::from_str::<serde_json::Value>(document)
            .map_err(|error| invalid(format!("MADE_WORKER_ROOT_POLICIES_JSON: {error}")))?;
        let object = object.as_object().ok_or_else(|| {
            invalid("MADE_WORKER_ROOT_POLICIES_JSON must be an object keyed by ceremony root")
        })?;
        let mut roots = BTreeMap::new();
        for (root, value) in object {
            let root_id = CeremonyId::new(root.clone())?;
            if roots.contains_key(&root_id) {
                return Err(invalid("duplicate canonical ceremony root policy"));
            }
            let fields = value
                .as_object()
                .ok_or_else(|| invalid(format!("root policy {root} must be a JSON object")))?;
            if fields.len() != 4
                || !["priority", "weight", "cost", "requested_capacity"]
                    .iter()
                    .all(|field| fields.contains_key(*field))
            {
                return Err(invalid(format!(
                    "root policy {root} requires exactly priority, weight, cost and requested_capacity"
                )));
            }
            let number = |field: &'static str| -> Result<u64, DomainError> {
                fields
                    .get(field)
                    .and_then(serde_json::Value::as_u64)
                    .ok_or_else(|| invalid(format!("root policy {root}.{field} must be unsigned")))
            };
            let priority = u16::try_from(number("priority")?)
                .map_err(|_| invalid(format!("root policy {root}.priority is too large")))?;
            let weight = u32::try_from(number("weight")?)
                .map_err(|_| invalid(format!("root policy {root}.weight is too large")))?;
            let cost = u32::try_from(number("cost")?)
                .map_err(|_| invalid(format!("root policy {root}.cost is too large")))?;
            let requested_capacity =
                u32::try_from(number("requested_capacity")?).map_err(|_| {
                    invalid(format!(
                        "root policy {root}.requested_capacity is too large"
                    ))
                })?;
            roots.insert(
                root_id,
                CeremonyWorkerRootPolicy::new(
                    CeremonyWorkerPriority::new(priority)?,
                    CeremonyWorkerWeight::new(weight)?,
                    CeremonyWorkerCost::new(cost)?,
                    CeremonyWorkerCapacity::new(requested_capacity)?,
                ),
            );
        }
        Ok(Self { default, roots })
    }
}

impl CeremonyWorkerRootPolicyPort for ConfiguredWorkerRootPolicy {
    fn policy_for(&self, root: &CeremonyId) -> CeremonyWorkerRootPolicy {
        self.roots.get(root).copied().unwrap_or(self.default)
    }
}

fn invalid(reason: impl Into<String>) -> DomainError {
    DomainError::InvalidDocument {
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_policy() -> CeremonyWorkerRootPolicy {
        CeremonyWorkerRootPolicy::new(
            CeremonyWorkerPriority::DEFAULT,
            CeremonyWorkerWeight::new(1).unwrap(),
            CeremonyWorkerCost::new(1).unwrap(),
            CeremonyWorkerCapacity::new(1).unwrap(),
        )
    }

    #[test]
    fn selects_trusted_policy_by_root_and_keeps_a_default() {
        let configured = ConfiguredWorkerRootPolicy::from_json(
            default_policy(),
            Some(r#"{"root-b":{"priority":7,"weight":3,"cost":2,"requested_capacity":2}}"#),
        )
        .unwrap();

        let selected = configured.policy_for(&CeremonyId::new("root-b").unwrap());
        assert_eq!(selected.priority().value(), 7);
        assert_eq!(selected.weight().value(), 3);
        assert_eq!(
            configured.policy_for(&CeremonyId::new("root-a").unwrap()),
            default_policy()
        );
    }

    #[test]
    fn canonical_root_ids_are_preserved_and_duplicate_aliases_are_rejected() {
        let configured = ConfiguredWorkerRootPolicy::from_json(
            default_policy(),
            Some(r#"{" root-b ":{"priority":7,"weight":3,"cost":2,"requested_capacity":2}}"#),
        )
        .unwrap();
        assert_eq!(
            configured
                .policy_for(&CeremonyId::new("root-b").unwrap())
                .priority()
                .value(),
            7
        );
        let duplicate = r#"{
            "root-b":{"priority":7,"weight":3,"cost":2,"requested_capacity":2},
            " root-b ":{"priority":1,"weight":1,"cost":1,"requested_capacity":1}
        }"#;
        assert!(ConfiguredWorkerRootPolicy::from_json(default_policy(), Some(duplicate)).is_err());
    }

    #[test]
    fn invalid_or_partial_policy_fails_closed() {
        assert!(ConfiguredWorkerRootPolicy::from_json(
            default_policy(),
            Some(r#"{"root-b":{"weight":3}}"#),
        )
        .is_err());
        assert!(ConfiguredWorkerRootPolicy::from_json(default_policy(), Some("[]")).is_err());
    }
}
