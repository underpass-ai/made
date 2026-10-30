use made_core::entities::ProtocolDefinition;
use made_core::value_objects::{ProtocolName, ProtocolVersion};

/// Domain source for one side of a protocol-definition comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolDefinitionSource {
    Published {
        name: ProtocolName,
        version: ProtocolVersion,
    },
    Supplied(Box<ProtocolDefinition>),
}

impl ProtocolDefinitionSource {
    #[must_use]
    pub fn published(name: ProtocolName, version: ProtocolVersion) -> Self {
        Self::Published { name, version }
    }

    #[must_use]
    pub fn supplied(definition: ProtocolDefinition) -> Self {
        Self::Supplied(Box::new(definition))
    }
}
