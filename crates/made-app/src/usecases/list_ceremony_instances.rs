use std::fmt;
use std::sync::Arc;

use made_core::entities::CeremonyInstance;
use made_core::error::DomainError;
use made_core::ports::CeremonyInstanceRepositoryPort;

/// Lists every ceremony instance known to the configured repository.
pub struct ListCeremonyInstancesUseCase {
    repository: Arc<dyn CeremonyInstanceRepositoryPort>,
}

impl fmt::Debug for ListCeremonyInstancesUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ListCeremonyInstancesUseCase")
            .finish()
    }
}

impl ListCeremonyInstancesUseCase {
    #[must_use]
    pub fn new(repository: Arc<dyn CeremonyInstanceRepositoryPort>) -> Self {
        Self { repository }
    }

    #[tracing::instrument(name = "list_ceremony_instances", skip_all)]
    pub async fn execute(&self) -> Result<Vec<CeremonyInstance>, DomainError> {
        self.repository.list().await
    }
}
