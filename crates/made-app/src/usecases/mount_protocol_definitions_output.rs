use made_core::entities::ProtocolDefinition;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountProtocolDefinitionsOutput {
    definitions: Vec<ProtocolDefinition>,
}

impl MountProtocolDefinitionsOutput {
    #[must_use]
    pub fn new(definitions: Vec<ProtocolDefinition>) -> Self {
        Self { definitions }
    }

    #[must_use]
    pub fn definitions(&self) -> &[ProtocolDefinition] {
        &self.definitions
    }
}
