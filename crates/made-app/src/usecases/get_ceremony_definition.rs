use std::fmt;
use std::sync::Arc;

use made_core::entities::CeremonyDefinition;
use made_core::error::DomainError;
use made_core::ports::CeremonyDefinitionRepositoryPort;
use made_core::value_objects::{CeremonyName, CeremonyVersion};

/// Retrieves one mounted ceremony definition by its domain identity.
pub struct GetCeremonyDefinitionUseCase {
    repository: Arc<dyn CeremonyDefinitionRepositoryPort>,
}

impl fmt::Debug for GetCeremonyDefinitionUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GetCeremonyDefinitionUseCase")
            .finish()
    }
}

impl GetCeremonyDefinitionUseCase {
    #[must_use]
    pub fn new(repository: Arc<dyn CeremonyDefinitionRepositoryPort>) -> Self {
        Self { repository }
    }

    #[tracing::instrument(name = "get_ceremony_definition", skip_all, fields(name = %name, version = %version))]
    pub async fn execute(
        &self,
        name: &CeremonyName,
        version: &CeremonyVersion,
    ) -> Result<CeremonyDefinition, DomainError> {
        self.repository.get(name, version).await
    }
}
