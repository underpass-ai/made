use serde::{Deserialize, Deserializer, Serialize};

use crate::error::DomainError;

use super::{CeremonyId, ChildDepth, ChildDepthBudget, ChildGroupId, ChildPosition};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CeremonyLineage {
    root_id: CeremonyId,
    parent_id: CeremonyId,
    group_id: ChildGroupId,
    position: ChildPosition,
    depth: ChildDepth,
    remaining_depth: ChildDepthBudget,
}

impl CeremonyLineage {
    pub fn new(
        root_id: CeremonyId,
        parent_id: CeremonyId,
        group_id: ChildGroupId,
        position: ChildPosition,
        depth: ChildDepth,
        remaining_depth: ChildDepthBudget,
    ) -> Result<Self, DomainError> {
        let lineage = Self {
            root_id,
            parent_id,
            group_id,
            position,
            depth,
            remaining_depth,
        };
        lineage.validate()?;
        Ok(lineage)
    }
    #[must_use]
    pub fn root_id(&self) -> &CeremonyId {
        &self.root_id
    }
    #[must_use]
    pub fn parent_id(&self) -> &CeremonyId {
        &self.parent_id
    }
    #[must_use]
    pub fn group_id(&self) -> &ChildGroupId {
        &self.group_id
    }
    #[must_use]
    pub const fn position(&self) -> ChildPosition {
        self.position
    }
    #[must_use]
    pub const fn depth(&self) -> ChildDepth {
        self.depth
    }
    #[must_use]
    pub const fn remaining_depth(&self) -> ChildDepthBudget {
        self.remaining_depth
    }

    pub fn validate(&self) -> Result<(), DomainError> {
        let root_is_parent = self.root_id == self.parent_id;
        if (self.depth == ChildDepth::FIRST) != root_is_parent {
            return Err(DomainError::InvalidDocument {
                reason: "child lineage root and parent are inconsistent with depth".to_owned(),
            });
        }
        if self.depth.get() + self.remaining_depth.get() > super::MaxChildDepth::SERVER_MAX.get() {
            return Err(DomainError::InvalidDocument {
                reason: "child lineage exceeds the server depth ceiling".to_owned(),
            });
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct UncheckedCeremonyLineage {
    root_id: CeremonyId,
    parent_id: CeremonyId,
    group_id: ChildGroupId,
    position: ChildPosition,
    depth: ChildDepth,
    remaining_depth: ChildDepthBudget,
}

impl<'de> Deserialize<'de> for CeremonyLineage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = UncheckedCeremonyLineage::deserialize(deserializer)?;
        Self::new(
            raw.root_id,
            raw.parent_id,
            raw.group_id,
            raw.position,
            raw.depth,
            raw.remaining_depth,
        )
        .map_err(serde::de::Error::custom)
    }
}
