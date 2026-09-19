use super::{ConnectorCapability, ConnectorKind};
use made_core::value_objects::ExecutionConnectorId;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConnectorContractError {
    #[error("connector `{connector_id}` is missing capabilities for {kind:?}: {missing:?}")]
    MissingCapabilities {
        connector_id: ExecutionConnectorId,
        kind: ConnectorKind,
        missing: Vec<ConnectorCapability>,
    },
    #[error("repository/script connector `{connector_id}` must declare recovery capability")]
    MissingRecoveryCapability { connector_id: ExecutionConnectorId },
    #[error("connector `{connector_id}` of kind {kind:?} must not declare recovery capability")]
    UnexpectedRecoveryCapability {
        connector_id: ExecutionConnectorId,
        kind: ConnectorKind,
    },
    #[error("connector `{connector_id}` has kind {actual:?}; expected {expected:?}")]
    WrongKind {
        connector_id: ExecutionConnectorId,
        expected: ConnectorKind,
        actual: ConnectorKind,
    },
}
