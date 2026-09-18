use serde::{Deserialize, Deserializer, Serialize};

use crate::error::DomainError;

use super::{
    ChildGroupId, ChildSpawnCoordinates, MaxChildDepth, MaxChildren, PlannedChild, StepClaimFence,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChildSpawnPlan {
    group_id: ChildGroupId,
    coordinates: ChildSpawnCoordinates,
    active_claim_fence: StepClaimFence,
    children: Vec<PlannedChild>,
    max_children: MaxChildren,
    max_depth: MaxChildDepth,
}

impl ChildSpawnPlan {
    pub fn new(
        group_id: ChildGroupId,
        coordinates: ChildSpawnCoordinates,
        active_claim_fence: StepClaimFence,
        children: Vec<PlannedChild>,
        max_children: MaxChildren,
        max_depth: MaxChildDepth,
    ) -> Result<Self, DomainError> {
        if children.is_empty() || children.len() > usize::from(max_children.get()) {
            return Err(DomainError::InvalidDocument {
                reason: "child spawn plan width is invalid".to_owned(),
            });
        }
        if children
            .iter()
            .enumerate()
            .any(|(position, child)| usize::from(child.position().get()) != position)
        {
            return Err(DomainError::InvalidDocument {
                reason: "child spawn plan positions must be contiguous declaration indexes"
                    .to_owned(),
            });
        }
        let parent_id = children
            .first()
            .expect("non-empty child plan checked above")
            .lineage()
            .parent_id();
        if group_id != ChildGroupId::derive(parent_id, &coordinates) {
            return Err(DomainError::InvalidDocument {
                reason: "child spawn group id does not match its parent and coordinates".to_owned(),
            });
        }
        let first_lineage = children
            .first()
            .expect("non-empty child plan checked above")
            .lineage();
        for child in &children {
            let expected_id =
                super::ChildCeremonyId::derive(&group_id, child.position())?.into_ceremony_id();
            if child.child_id() != &expected_id
                || child.lineage().group_id() != &group_id
                || child.lineage().position() != child.position()
                || child.lineage().root_id() != first_lineage.root_id()
                || child.lineage().parent_id() != first_lineage.parent_id()
                || child.lineage().depth() != first_lineage.depth()
                || child.lineage().remaining_depth() != first_lineage.remaining_depth()
            {
                return Err(DomainError::InvalidDocument {
                    reason: "planned child identity or lineage is inconsistent".to_owned(),
                });
            }
        }
        Ok(Self {
            group_id,
            coordinates,
            active_claim_fence,
            children,
            max_children,
            max_depth,
        })
    }
    #[must_use]
    pub fn group_id(&self) -> &ChildGroupId {
        &self.group_id
    }
    #[must_use]
    pub fn coordinates(&self) -> &ChildSpawnCoordinates {
        &self.coordinates
    }
    #[must_use]
    pub fn active_claim_fence(&self) -> &StepClaimFence {
        &self.active_claim_fence
    }
    #[must_use]
    pub fn children(&self) -> &[PlannedChild] {
        &self.children
    }
    #[must_use]
    pub const fn max_children(&self) -> MaxChildren {
        self.max_children
    }
    #[must_use]
    pub const fn max_depth(&self) -> MaxChildDepth {
        self.max_depth
    }
}

impl<'de> Deserialize<'de> for ChildSpawnPlan {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct UncheckedChildSpawnPlan {
            group_id: ChildGroupId,
            coordinates: ChildSpawnCoordinates,
            active_claim_fence: StepClaimFence,
            children: Vec<PlannedChild>,
            max_children: MaxChildren,
            max_depth: MaxChildDepth,
        }

        let raw = UncheckedChildSpawnPlan::deserialize(deserializer)?;
        Self::new(
            raw.group_id,
            raw.coordinates,
            raw.active_claim_fence,
            raw.children,
            raw.max_children,
            raw.max_depth,
        )
        .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_objects::{
        CeremonyContext, CeremonyDefinitionDigest, CeremonyId, CeremonyLineage, CeremonyName,
        CeremonyVersion, ChildCeremonyId, ChildDepth, ChildDepthBudget, ChildPosition,
        StateIteration, StateVisit, StepId, StepIteration,
    };
    use serde_json::json;
    use time::OffsetDateTime;

    fn valid_plan() -> ChildSpawnPlan {
        let parent = CeremonyId::new("parent").unwrap();
        let coordinates = ChildSpawnCoordinates::new(
            StepId::new("spawn").unwrap(),
            StateVisit::FIRST,
            StateIteration::FIRST,
            StepIteration::FIRST,
        );
        let group_id = ChildGroupId::derive(&parent, &coordinates);
        let children = (0..2)
            .map(|position| {
                let position = ChildPosition::new(position);
                super::PlannedChild::new(
                    ChildCeremonyId::derive(&group_id, position)
                        .unwrap()
                        .into_ceremony_id(),
                    position,
                    CeremonyName::new("review_child").unwrap(),
                    CeremonyVersion::v1(),
                    CeremonyDefinitionDigest::from_bytes([7; 32]),
                    CeremonyContext::empty(),
                    CeremonyLineage::new(
                        parent.clone(),
                        parent.clone(),
                        group_id.clone(),
                        position,
                        ChildDepth::FIRST,
                        ChildDepthBudget::new(2).unwrap(),
                    )
                    .unwrap(),
                    None,
                    OffsetDateTime::UNIX_EPOCH,
                )
            })
            .collect();
        ChildSpawnPlan::new(
            group_id,
            coordinates,
            StepClaimFence::new("a".repeat(64)).unwrap(),
            children,
            MaxChildren::new(2).unwrap(),
            MaxChildDepth::new(3).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn deserialization_rechecks_derived_group_and_child_identity() {
        let plan = valid_plan();
        let mut value = serde_json::to_value(plan).unwrap();
        value["group_id"] = json!("0".repeat(64));
        assert!(serde_json::from_value::<ChildSpawnPlan>(value).is_err());

        let mut value = serde_json::to_value(valid_plan()).unwrap();
        value["children"][1]["position"] = json!(0);
        assert!(serde_json::from_value::<ChildSpawnPlan>(value).is_err());

        let mut value = serde_json::to_value(valid_plan()).unwrap();
        value["coordinates"]["state_visit"] = json!(2);
        assert!(
            serde_json::from_value::<ChildSpawnPlan>(value).is_err(),
            "a plan from another state visit kept the old group identity"
        );

        let mut value = serde_json::to_value(valid_plan()).unwrap();
        value["active_claim_fence"] = json!("not-a-canonical-fence");
        assert!(serde_json::from_value::<ChildSpawnPlan>(value).is_err());
    }

    #[test]
    fn deserialization_rechecks_lineage_budget_and_parent_relation() {
        let mut value = serde_json::to_value(valid_plan()).unwrap();
        value["children"][0]["lineage"]["root_id"] = json!("foreign-root");
        assert!(serde_json::from_value::<ChildSpawnPlan>(value).is_err());

        let malformed = json!({
            "root_id": "root",
            "parent_id": "different-parent",
            "group_id": "b".repeat(64),
            "position": 0,
            "depth": 1,
            "remaining_depth": 1
        });
        assert!(serde_json::from_value::<CeremonyLineage>(malformed).is_err());
    }
}
