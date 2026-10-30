use std::fmt;
use std::sync::Arc;

use made_core::entities::ProtocolDefinition;
use made_core::error::DomainError;
use made_core::ports::ProtocolDefinitionRepositoryPort;

/// Lists every mounted protocol definition.
pub struct ListProtocolDefinitionsUseCase {
    repository: Arc<dyn ProtocolDefinitionRepositoryPort>,
}

impl fmt::Debug for ListProtocolDefinitionsUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ListProtocolDefinitionsUseCase")
            .finish()
    }
}

impl ListProtocolDefinitionsUseCase {
    #[must_use]
    pub fn new(repository: Arc<dyn ProtocolDefinitionRepositoryPort>) -> Self {
        Self { repository }
    }

    #[tracing::instrument(name = "list_protocol_definitions", skip_all)]
    pub async fn execute(&self) -> Result<Vec<ProtocolDefinition>, DomainError> {
        self.repository.list().await
    }
}
