use made_core::error::DomainError;
use made_core::value_objects::CeremonyId;

/// Failure to inspect or claim one ceremony while its sibling claims survive.
#[derive(Debug, Clone, PartialEq)]
pub struct CeremonyWorkClaimFailure {
    ceremony_id: CeremonyId,
    error: DomainError,
}

impl CeremonyWorkClaimFailure {
    #[must_use]
    pub const fn new(ceremony_id: CeremonyId, error: DomainError) -> Self {
        Self { ceremony_id, error }
    }

    #[must_use]
    pub const fn ceremony_id(&self) -> &CeremonyId {
        &self.ceremony_id
    }

    #[must_use]
    pub const fn error(&self) -> &DomainError {
        &self.error
    }
}
