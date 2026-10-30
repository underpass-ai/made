use async_trait::async_trait;
use made_adapters::yaml::ProtocolDefinitionYaml;
use made_core::entities::ProtocolDefinition;
use made_core::error::DomainError;
use made_core::ports::ProtocolDefinitionSourcePort;

/// Supplies caller-owned protocol definitions to the application layer.
#[derive(Debug, Clone)]
pub struct InProcessProtocolDefinitionSource {
    definitions: Vec<ProtocolDefinition>,
}

impl InProcessProtocolDefinitionSource {
    #[must_use]
    pub fn new(definitions: impl IntoIterator<Item = ProtocolDefinition>) -> Self {
        Self {
            definitions: definitions.into_iter().collect(),
        }
    }

    pub fn from_yaml(raw: &str) -> Result<Self, DomainError> {
        ProtocolDefinitionYaml::parse_str(raw).map(|definition| Self::new([definition]))
    }

    pub(crate) fn definitions(&self) -> &[ProtocolDefinition] {
        &self.definitions
    }
}

#[async_trait]
impl ProtocolDefinitionSourcePort for InProcessProtocolDefinitionSource {
    async fn load(&self) -> Result<Vec<ProtocolDefinition>, DomainError> {
        Ok(self.definitions.clone())
    }
}
