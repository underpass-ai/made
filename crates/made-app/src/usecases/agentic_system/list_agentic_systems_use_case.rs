//! [`ListAgenticSystemsUseCase`] — the catalogue of designs.

use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::{AgenticSystemPage, AgenticSystemQuery, AgenticSystemRepositoryPort};

pub struct ListAgenticSystemsUseCase {
    repository: Arc<dyn AgenticSystemRepositoryPort>,
}

impl std::fmt::Debug for ListAgenticSystemsUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("ListAgenticSystemsUseCase").finish()
    }
}

impl ListAgenticSystemsUseCase {
    #[must_use]
    pub const fn new(repository: Arc<dyn AgenticSystemRepositoryPort>) -> Self {
        Self { repository }
    }

    /// One bounded page of heads.
    #[tracing::instrument(name = "list_agentic_systems", skip_all)]
    pub async fn execute(
        &self,
        query: &AgenticSystemQuery,
    ) -> Result<AgenticSystemPage, DomainError> {
        self.repository.list(query).await
    }
}
