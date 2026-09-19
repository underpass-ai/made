use made_core::value_objects::ExecutionOperationId;
use thiserror::Error;

use super::contract::ConnectorContractError;

/// Safe adapter error vocabulary.  Backend details are intentionally not
/// carried here: paths, command lines, response bodies and credentials must
/// stay in the backend's private logs, never cross this boundary.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConnectorError {
    #[error("connector contract rejected: {0}")]
    Contract(#[from] ConnectorContractError),
    #[error("execution intent is invalid")]
    InvalidIntent,
    #[error("execution intent does not belong to this connector")]
    ConnectorMismatch,
    #[error("receipt field `{field}` does not match the execution intent")]
    ReceiptMismatch { field: &'static str },
    #[error("repository/script execution failed")]
    RepositoryScript,
    #[error("filesystem receipt operation failed")]
    Filesystem,
    #[error("receipt transport delivery failed")]
    Transport,
    #[error("receipt for operation `{0}` conflicts with the durable record")]
    ReceiptConflict(ExecutionOperationId),
    #[error("receipt serialization failed")]
    ReceiptSerialization,
    #[error("durable operation `{0}` requires reconciliation")]
    ReconciliationRequired(ExecutionOperationId),
}
