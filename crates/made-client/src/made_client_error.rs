use std::io;

use thiserror::Error;
use tonic::Code;

/// Errors a reference client can act on without inspecting server internals.
#[derive(Debug, Error)]
pub enum MadeClientError {
    #[error("invalid MADE endpoint: {0}")]
    InvalidEndpoint(String),
    #[error("invalid x-made-request-id: {0}")]
    InvalidRequestId(String),
    #[error("invalid TLS client configuration: {0}")]
    TlsConfiguration(String),
    #[error("connection attempts exhausted: {0}")]
    ConnectionExhausted(String),
    #[error("gRPC transport failed: {0}")]
    Transport(String),
    #[error("MADE returned {code:?}: {message}")]
    RemoteStatus { code: Code, message: String },
    #[error("public protocol violation: {0}")]
    ProtocolViolation(String),
    #[error("checkpoint belongs to {actual}, expected {expected}")]
    CursorScopeMismatch { expected: String, actual: String },
    #[error("cursor checkpoint is corrupt: {0}")]
    CursorCheckpointCorrupt(String),
    #[error("artifact digest mismatch: expected {expected}, observed {observed}")]
    ArtifactIntegrityMismatch { expected: String, observed: String },
    #[error("artifact size mismatch: expected {expected}, observed {observed}")]
    ArtifactSizeMismatch { expected: u64, observed: u64 },
    #[error("local I/O failed at {path}: {source}")]
    Io { path: String, source: io::Error },
}

impl MadeClientError {
    #[must_use]
    pub fn is_retryable_read(&self) -> bool {
        matches!(
            self,
            Self::RemoteStatus {
                code: Code::Unavailable | Code::DeadlineExceeded,
                ..
            } | Self::Transport(_)
        )
    }

    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::InvalidEndpoint(_)
            | Self::InvalidRequestId(_)
            | Self::TlsConfiguration(_)
            | Self::ConnectionExhausted(_)
            | Self::Transport(_)
            | Self::RemoteStatus { .. } => 3,
            Self::ProtocolViolation(_)
            | Self::CursorScopeMismatch { .. }
            | Self::CursorCheckpointCorrupt(_)
            | Self::ArtifactIntegrityMismatch { .. }
            | Self::ArtifactSizeMismatch { .. } => 4,
            Self::Io { .. } => 5,
        }
    }

    pub(crate) fn from_status(status: tonic::Status) -> Self {
        const MAX_MESSAGE_CHARS: usize = 512;
        let code = status.code();
        let message: String = status.message().chars().take(MAX_MESSAGE_CHARS).collect();
        drop(status);
        Self::RemoteStatus { code, message }
    }

    pub(crate) fn io(path: &std::path::Path, source: io::Error) -> Self {
        Self::Io {
            path: path.display().to_string(),
            source,
        }
    }
}
