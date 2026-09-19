use thiserror::Error;
use tonic::Status;

/// Why a mutually authenticated channel did not resolve to a MADE principal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MutualTlsAuthenticationError {
    #[error("a verified client certificate is required")]
    MissingCertificate,
    #[error("the client certificate chain is empty")]
    EmptyCertificateChain,
    #[error("the verified client certificate is not mapped to a principal")]
    UnmappedCertificate,
}

impl From<MutualTlsAuthenticationError> for Status {
    fn from(error: MutualTlsAuthenticationError) -> Self {
        Self::unauthenticated(error.to_string())
    }
}
