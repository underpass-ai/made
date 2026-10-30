use made_core::DomainError;

#[derive(Debug)]
pub(crate) enum ProtocolWorkerClaimError {
    Permission(DomainError),
    Budget(DomainError),
    Failure(DomainError),
}

impl ProtocolWorkerClaimError {
    pub(crate) fn into_domain(self) -> DomainError {
        match self {
            Self::Permission(error) | Self::Budget(error) | Self::Failure(error) => error,
        }
    }
}

impl From<DomainError> for ProtocolWorkerClaimError {
    fn from(error: DomainError) -> Self {
        Self::Failure(error)
    }
}
