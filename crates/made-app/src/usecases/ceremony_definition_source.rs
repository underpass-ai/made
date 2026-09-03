use made_core::entities::CeremonyDefinition;
use made_core::value_objects::{CeremonyName, CeremonyVersion};

/// Domain source for one side of a ceremony-definition comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CeremonyDefinitionSource {
    Published {
        name: CeremonyName,
        version: CeremonyVersion,
    },
    Supplied(Box<CeremonyDefinition>),
}

impl CeremonyDefinitionSource {
    #[must_use]
    pub fn published(name: CeremonyName, version: CeremonyVersion) -> Self {
        Self::Published { name, version }
    }

    #[must_use]
    pub fn supplied(definition: CeremonyDefinition) -> Self {
        Self::Supplied(Box::new(definition))
    }
}
