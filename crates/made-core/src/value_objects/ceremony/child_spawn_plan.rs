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

#[derive(Deserialize)]
struct UncheckedChildSpawnPlan {
    group_id: ChildGroupId,
    coordinates: ChildSpawnCoordinates,
    active_claim_fence: StepClaimFence,
    children: Vec<PlannedChild>,
    max_children: MaxChildren,
    max_depth: MaxChildDepth,
}

impl<'de> Deserialize<'de> for ChildSpawnPlan {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
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
