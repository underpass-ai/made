use made_core::DomainError;
use thiserror::Error;
use tonic::Status;

use super::MutualTlsAuthenticationError;

/// Configuration and invocation errors raised before policy evaluation.
#[derive(Debug, Error)]
pub enum GrpcAuthorizationError {
    #[error("a trusted gRPC fixture must use an explicit local-host principal")]
    InvalidTrustedHost,
    #[error("trusted gRPC request namespace must not be empty")]
    EmptyRequestNamespace,
    #[error("the gRPC authorization boundary is not configured")]
    Unconfigured,
    #[error("x-made-request-id is required for remote gRPC calls")]
    MissingRequestId,
    #[error("x-made-request-id must be printable ASCII")]
    InvalidRequestIdEncoding,
    #[error("invalid x-made-request-id: {0}")]
    InvalidRequestId(#[source] DomainError),
    #[error("x-made-target-digest is accepted only from an explicitly trusted MCP proxy")]
    UntrustedTargetDigestProxy,
    #[error("x-made-target-digest must be printable ASCII")]
    InvalidTargetDigestEncoding,
    #[error("invalid x-made-target-digest: {0}")]
    InvalidTargetDigest(#[source] DomainError),
    #[error(transparent)]
    MutualTls(#[from] MutualTlsAuthenticationError),
}

impl From<GrpcAuthorizationError> for Status {
    fn from(error: GrpcAuthorizationError) -> Self {
        match error {
            GrpcAuthorizationError::MutualTls(error) => error.into(),
            GrpcAuthorizationError::MissingRequestId
            | GrpcAuthorizationError::InvalidRequestIdEncoding
            | GrpcAuthorizationError::InvalidRequestId(_)
            | GrpcAuthorizationError::InvalidTargetDigestEncoding
            | GrpcAuthorizationError::InvalidTargetDigest(_) => {
                Self::invalid_argument(error.to_string())
            }
            GrpcAuthorizationError::UntrustedTargetDigestProxy => {
                Self::permission_denied(error.to_string())
            }
            GrpcAuthorizationError::InvalidTrustedHost
            | GrpcAuthorizationError::EmptyRequestNamespace
            | GrpcAuthorizationError::Unconfigured => Self::failed_precondition(error.to_string()),
        }
    }
}
