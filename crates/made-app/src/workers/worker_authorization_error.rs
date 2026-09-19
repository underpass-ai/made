use made_core::DomainError;

#[derive(Debug)]
pub enum WorkerAuthorizationError {
    Denied(DomainError),
    Failure(DomainError),
}

impl WorkerAuthorizationError {
    #[must_use]
    pub fn into_domain(self) -> DomainError {
        match self {
            Self::Denied(error) | Self::Failure(error) => error,
        }
    }
}
