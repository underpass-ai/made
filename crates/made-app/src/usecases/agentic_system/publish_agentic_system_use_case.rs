//! [`PublishAgenticSystemUseCase`] — seal a revision a run can pin.

use std::sync::Arc;

use made_core::entities::{AgenticSystemPublicationOutcome, PublishedAgenticSystem};
use made_core::error::DomainError;
use made_core::ports::{
    AgenticSystemPublicationPort, AgenticSystemRepositoryPort, AgenticSystemSaveOutcome, ClockPort,
};
use made_core::value_objects::{AgenticSystemId, AgenticSystemLifecycle, AgenticSystemRevision};

use super::{AgenticSystemPublicationView, ValidateAgenticSystemUseCase};

/// Validates, seals, and then records that the design is sealed.
///
/// In that order, and the order is the contract. Recording first would
/// leave a design marked published that the seal then refused;
/// sealing without validating would fix bytes nobody had checked
/// against the ceremonies they compose.
///
/// Nothing here is a transaction across two stores, and it does not
/// need to be. The seal is idempotent by content: a retry after a
/// crash between the two steps answers `already_published` and
/// finishes the job, and two callers sealing different content under
/// one revision are separated by the publication store itself.
pub struct PublishAgenticSystemUseCase {
    repository: Arc<dyn AgenticSystemRepositoryPort>,
    publications: Arc<dyn AgenticSystemPublicationPort>,
    validate: Arc<ValidateAgenticSystemUseCase>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for PublishAgenticSystemUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PublishAgenticSystemUseCase")
            .finish()
    }
}

impl PublishAgenticSystemUseCase {
    #[must_use]
    pub const fn new(
        repository: Arc<dyn AgenticSystemRepositoryPort>,
        publications: Arc<dyn AgenticSystemPublicationPort>,
        validate: Arc<ValidateAgenticSystemUseCase>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            repository,
            publications,
            validate,
            clock,
        }
    }

    #[tracing::instrument(
        name = "publish_agentic_system",
        skip_all,
        fields(agentic_system_id = %id, revision = %expected_revision)
    )]
    pub async fn execute(
        &self,
        id: &AgenticSystemId,
        expected_revision: AgenticSystemRevision,
    ) -> Result<AgenticSystemPublicationView, DomainError> {
        let system = self
            .repository
            .get(id, Some(expected_revision))
            .await?
            .ok_or(DomainError::NotFound {
                what: "agentic_system",
            })?;
        if system.lifecycle() == AgenticSystemLifecycle::Deprecated {
            return Err(DomainError::InvalidTransition {
                from: system.lifecycle().as_str(),
                to: "published",
            });
        }
        let validation = self.validate.of(system.clone()).await?;
        if !validation.is_publishable() {
            return Err(DomainError::InvalidDocument {
                reason: format!(
                    "this design cannot be published: {}",
                    validation.report().blocking_summary()
                ),
            });
        }

        let now = self.clock.now();
        let sealed = PublishedAgenticSystem::seal(published_form(&system, now)?, now)?;
        let outcome = self.publications.publish(sealed).await?;
        if let AgenticSystemPublicationOutcome::RevisionOccupied { published, offered } = &outcome {
            return Err(DomainError::InvalidDocument {
                reason: format!(
                    "revision {expected_revision} already holds a different design ({published}); \
                     this one is {offered}. Edit it as a new revision instead"
                ),
            });
        }

        let head = if outcome.is_new() {
            self.record_seal(&system, expected_revision, now).await?
        } else {
            expected_revision
        };
        Ok(AgenticSystemPublicationView::new(
            outcome,
            validation,
            expected_revision,
            head,
        ))
    }

    /// Advance the head so the catalogue can say this design is
    /// published.
    ///
    /// A conflict here means somebody edited the design between the
    /// validation and now. The seal stands — it is immutable and was
    /// taken over the revision that was checked — so the refusal says
    /// so rather than implying the publication did not happen.
    async fn record_seal(
        &self,
        system: &made_core::entities::AgenticSystem,
        expected_revision: AgenticSystemRevision,
        now: time::OffsetDateTime,
    ) -> Result<AgenticSystemRevision, DomainError> {
        match self
            .repository
            .save(published_form(system, now)?, Some(expected_revision))
            .await?
        {
            AgenticSystemSaveOutcome::Saved { revision } => Ok(revision),
            AgenticSystemSaveOutcome::RevisionConflict { current } => {
                Err(DomainError::InvalidDocument {
                    reason: format!(
                        "revision {expected_revision} was sealed, but the design has since moved \
                         to revision {current}, so the catalogue still shows it as a draft"
                    ),
                })
            }
        }
    }
}

/// The design marked published, or as it already is when it is.
fn published_form(
    system: &made_core::entities::AgenticSystem,
    now: time::OffsetDateTime,
) -> Result<made_core::entities::AgenticSystem, DomainError> {
    match system.lifecycle() {
        AgenticSystemLifecycle::Draft => system.published(now),
        AgenticSystemLifecycle::Published | AgenticSystemLifecycle::Deprecated => {
            Ok(system.clone())
        }
    }
}
