use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeExecutorConnectError {
    #[error("invalid runtime gRPC endpoint")]
    InvalidEndpoint,

    #[error("runtime gRPC connection failed: {0}")]
    Transport(#[from] tonic::transport::Error),

    #[error("failed to read runtime TLS material at {path}: {source}")]
    TlsReadFailed {
        path: String,
        #[source]
        source: std::io::Error,
    },
}
