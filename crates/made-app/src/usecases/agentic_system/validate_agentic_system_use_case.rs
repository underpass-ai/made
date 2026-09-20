//! [`ValidateAgenticSystemUseCase`] — compare a design against reality.

use std::sync::Arc;

use made_core::entities::AgenticSystem;
use made_core::error::DomainError;
use made_core::ports::AgenticSystemRepositoryPort;
use made_core::value_objects::{AgenticSystemId, AgenticSystemRevision};

use super::{AgenticSystemPins, AgenticSystemValidationView};

/// Resolves every pin and runs the analysis over what came back.
pub struct ValidateAgenticSystemUseCase {
    repository: Arc<dyn AgenticSystemRepositoryPort>,
    pins: Arc<AgenticSystemPins>,
}

impl std::fmt::Debug for ValidateAgenticSystemUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ValidateAgenticSystemUseCase")
            .finish()
    }
}

impl ValidateAgenticSystemUseCase {
    #[must_use]
    pub const fn new(
        repository: Arc<dyn AgenticSystemRepositoryPort>,
        pins: Arc<AgenticSystemPins>,
    ) -> Self {
        Self { repository, pins }
    }

    #[tracing::instrument(
        name = "validate_agentic_system",
        skip_all,
        fields(agentic_system_id = %id)
    )]
    pub async fn execute(
        &self,
        id: &AgenticSystemId,
        revision: Option<AgenticSystemRevision>,
    ) -> Result<AgenticSystemValidationView, DomainError> {
        let system = self
            .repository
            .get(id, revision)
            .await?
            .ok_or(DomainError::NotFound {
                what: "agentic_system",
            })?;
        self.of(system).await
    }

    /// Analyse a design the caller already has.
    ///
    /// Shared with publication rather than re-implemented there: the
    /// check that decides whether a revision may be sealed has to be
    /// the same one the author was shown.
    pub async fn of(
        &self,
        system: AgenticSystem,
    ) -> Result<AgenticSystemValidationView, DomainError> {
        let resolved = self.pins.resolve(&system).await?;
        let report = system.analyze(&resolved);
        Ok(AgenticSystemValidationView::new(system, report))
    }
}
