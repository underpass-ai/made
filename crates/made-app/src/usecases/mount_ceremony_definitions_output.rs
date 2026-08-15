use made_core::entities::CeremonyDefinition;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountCeremonyDefinitionsOutput {
    definitions: Vec<CeremonyDefinition>,
}

impl MountCeremonyDefinitionsOutput {
    #[must_use]
    pub fn new(definitions: Vec<CeremonyDefinition>) -> Self {
        Self { definitions }
    }

    #[must_use]
    pub fn definitions(&self) -> &[CeremonyDefinition] {
        &self.definitions
    }
}
