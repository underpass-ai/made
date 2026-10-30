use std::fmt;
use std::sync::Arc;

use made_core::entities::ProtocolDefinition;
use made_core::error::DomainError;
use made_core::ports::ProtocolDefinitionRepositoryPort;
use made_core::value_objects::{ProtocolName, ProtocolVersion};

/// Retrieves one mounted protocol definition by its domain identity.
pub struct GetProtocolDefinitionUseCase {
    repository: Arc<dyn ProtocolDefinitionRepositoryPort>,
}

impl fmt::Debug for GetProtocolDefinitionUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GetProtocolDefinitionUseCase")
            .finish()
    }
}

impl GetProtocolDefinitionUseCase {
    #[must_use]
    pub fn new(repository: Arc<dyn ProtocolDefinitionRepositoryPort>) -> Self {
        Self { repository }
    }

    #[tracing::instrument(name = "get_protocol_definition", skip_all, fields(name = %name, version = %version))]
    pub async fn execute(
        &self,
        name: &ProtocolName,
        version: &ProtocolVersion,
    ) -> Result<ProtocolDefinition, DomainError> {
        self.repository.get(name, version).await
    }
}
