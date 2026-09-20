//! [`DesignAgenticSystemUseCase`] — write a system down.

use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::{AgenticSystemRepositoryPort, AgenticSystemSaveOutcome, ClockPort};

use super::{AgenticSystemDesignDocument, AgenticSystemPins, AgenticSystemView};

/// Turns an author's document into a stored revision.
///
/// Nothing is validated against published reality here beyond filling
/// in the digests the author left out. A design is something somebody
/// is still working on, and refusing to write down a half-finished
/// system would mean the only way to get advice about one is to have
/// finished it.
pub struct DesignAgenticSystemUseCase {
    repository: Arc<dyn AgenticSystemRepositoryPort>,
    pins: Arc<AgenticSystemPins>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for DesignAgenticSystemUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DesignAgenticSystemUseCase")
            .finish()
    }
}

impl DesignAgenticSystemUseCase {
    #[must_use]
    pub const fn new(
        repository: Arc<dyn AgenticSystemRepositoryPort>,
        pins: Arc<AgenticSystemPins>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            repository,
            pins,
            clock,
        }
    }

    /// Store the document as the next revision of its design.
    ///
    /// The expected revision is the author's: `None` says "this system
    /// does not exist yet", and anything else says which revision they
    /// were looking at. A conflict is returned as a refusal naming the
    /// revision that is actually current, so the author can read what
    /// they missed instead of guessing.
    #[tracing::instrument(
        name = "design_agentic_system",
        skip_all,
        fields(agentic_system_id = %document.id())
    )]
    pub async fn execute(
        &self,
        document: AgenticSystemDesignDocument,
    ) -> Result<AgenticSystemView, DomainError> {
        let resolved = self.pins.digests(&document).await?;
        let expected = document.expected_revision();
        let draft = document.into_draft(&resolved, self.clock.now())?;
        match self.repository.save(draft.clone(), expected).await? {
            AgenticSystemSaveOutcome::Saved { revision } => {
                AgenticSystemView::of(draft.at_revision(revision))
            }
            AgenticSystemSaveOutcome::RevisionConflict { current } => {
                Err(DomainError::InvalidDocument {
                    reason: format!(
                        "this design is at revision {current}; read it and apply your change to \
                         that revision before saving"
                    ),
                })
            }
        }
    }
}
