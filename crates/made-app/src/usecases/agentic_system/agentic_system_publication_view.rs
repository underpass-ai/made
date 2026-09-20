use made_core::entities::AgenticSystemPublicationOutcome;
use made_core::value_objects::{AgenticSystemDigest, AgenticSystemRevision};

use super::AgenticSystemValidationView;

/// What happened when a revision was sealed.
///
/// The validation carried out beforehand travels with the answer,
/// because "published" without it is a claim nobody can check, and a
/// refusal without it is a refusal nobody can act on.
#[derive(Debug, Clone, PartialEq)]
pub struct AgenticSystemPublicationView {
    outcome: AgenticSystemPublicationOutcome,
    validation: AgenticSystemValidationView,
    sealed_revision: AgenticSystemRevision,
    head_revision: AgenticSystemRevision,
}

impl AgenticSystemPublicationView {
    #[must_use]
    pub const fn new(
        outcome: AgenticSystemPublicationOutcome,
        validation: AgenticSystemValidationView,
        sealed_revision: AgenticSystemRevision,
        head_revision: AgenticSystemRevision,
    ) -> Self {
        Self {
            outcome,
            validation,
            sealed_revision,
            head_revision,
        }
    }

    #[must_use]
    pub const fn outcome(&self) -> &AgenticSystemPublicationOutcome {
        &self.outcome
    }

    #[must_use]
    pub const fn validation(&self) -> &AgenticSystemValidationView {
        &self.validation
    }

    /// The revision a run may now pin.
    #[must_use]
    pub const fn sealed_revision(&self) -> AgenticSystemRevision {
        self.sealed_revision
    }

    /// Where the design's history stands after the seal was recorded.
    #[must_use]
    pub const fn head_revision(&self) -> AgenticSystemRevision {
        self.head_revision
    }

    #[must_use]
    pub fn digest(&self) -> Option<AgenticSystemDigest> {
        self.outcome
            .published()
            .map(made_core::entities::PublishedAgenticSystem::digest)
    }
}
