use made_core::error::DomainError;
use made_core::value_objects::{CeremonyId, StepId};

/// Failure of one accepted worker item without discarding sibling outcomes.
#[derive(Debug, Clone, PartialEq)]
pub struct CeremonyWorkerItemFailure {
    ceremony_id: CeremonyId,
    step_id: StepId,
    error: DomainError,
}

impl CeremonyWorkerItemFailure {
    #[must_use]
    pub const fn new(ceremony_id: CeremonyId, step_id: StepId, error: DomainError) -> Self {
        Self {
            ceremony_id,
            step_id,
            error,
        }
    }

    #[must_use]
    pub const fn ceremony_id(&self) -> &CeremonyId {
        &self.ceremony_id
    }

    #[must_use]
    pub const fn step_id(&self) -> &StepId {
        &self.step_id
    }

    #[must_use]
    pub const fn error(&self) -> &DomainError {
        &self.error
    }
}
