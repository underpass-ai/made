use made_core::error::DomainError;
use made_core::value_objects::{CeremonyChildSpawn, MaxChildDepth, MaxChildren};
use serde::Deserialize;

use super::ChildSpecIntent;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ChildSpawnIntent {
    children: Vec<ChildSpecIntent>,
    max_children: u64,
    max_depth: u64,
}

impl ChildSpawnIntent {
    pub(super) fn into_domain(self) -> Result<CeremonyChildSpawn, DomainError> {
        CeremonyChildSpawn::new(
            self.children
                .into_iter()
                .map(ChildSpecIntent::into_domain)
                .collect::<Result<Vec<_>, _>>()?,
            MaxChildren::new(u16::try_from(self.max_children).unwrap_or(u16::MAX))?,
            MaxChildDepth::new(u16::try_from(self.max_depth).unwrap_or(u16::MAX))?,
        )
    }
}
