//! [`GetAgenticSystemUseCase`] — read one design back.

use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::AgenticSystemRepositoryPort;
use made_core::value_objects::{AgenticSystemId, AgenticSystemRevision};

use super::AgenticSystemView;

pub struct GetAgenticSystemUseCase {
    repository: Arc<dyn AgenticSystemRepositoryPort>,
}

impl std::fmt::Debug for GetAgenticSystemUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("GetAgenticSystemUseCase").finish()
    }
}

impl GetAgenticSystemUseCase {
    #[must_use]
    pub const fn new(repository: Arc<dyn AgenticSystemRepositoryPort>) -> Self {
        Self { repository }
    }

    /// One design, at a named revision or at its head.
    ///
    /// Reading an earlier revision is not a curiosity: it is how
    /// somebody sees what a run was pinned to after the design has
    /// moved on.
    #[tracing::instrument(name = "get_agentic_system", skip_all, fields(agentic_system_id = %id))]
    pub async fn execute(
        &self,
        id: &AgenticSystemId,
        revision: Option<AgenticSystemRevision>,
    ) -> Result<AgenticSystemView, DomainError> {
        let system = self
            .repository
            .get(id, revision)
            .await?
            .ok_or(DomainError::NotFound {
                what: "agentic_system",
            })?;
        AgenticSystemView::of(system)
    }
}
