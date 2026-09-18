use thiserror::Error;

use crate::DomainError;

/// Stable failure vocabulary for artifact storage and transfer.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ArtifactStoreError {
    #[error(transparent)]
    Invalid(#[from] DomainError),
    #[error("artifact or upload was not found")]
    NotFound,
    #[error("artifact size {actual} exceeds maximum {max}")]
    ArtifactTooLarge { actual: u64, max: u64 },
    #[error("chunk size {actual} exceeds maximum {max}")]
    ChunkTooLarge { actual: usize, max: u32 },
    #[error("expected upload offset {expected}, got {actual}")]
    UnexpectedOffset { expected: u64, actual: u64 },
    #[error("chunk digest does not match its bytes")]
    ChunkDigestMismatch,
    #[error("artifact is incomplete: expected {expected} bytes, stored {actual}")]
    Incomplete { expected: u64, actual: u64 },
    #[error("artifact digest does not match its complete content")]
    FinalDigestMismatch,
    #[error("upload was already committed")]
    UploadCommitted,
    #[error("upload was aborted")]
    UploadAborted,
    #[error("idempotency key was already used with different upload metadata")]
    IdempotencyConflict,
    #[error("artifact content was retired by retention policy")]
    Tombstoned,
    #[error("artifact operation is denied at the host trust boundary")]
    AccessDenied,
    #[error("artifact storage is unavailable")]
    StorageUnavailable,
    #[error("artifact backup is corrupt or incomplete")]
    InvalidBackup,
}
