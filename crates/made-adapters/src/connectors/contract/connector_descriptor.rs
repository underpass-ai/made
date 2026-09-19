use super::{ConnectorCapabilities, ConnectorCapability, ConnectorContractError, ConnectorKind};
use made_core::value_objects::{ExecutionConnectorId, ExecutionRecoveryCapability};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorDescriptor {
    id: ExecutionConnectorId,
    kind: ConnectorKind,
    capabilities: ConnectorCapabilities,
    recovery_capability: Option<ExecutionRecoveryCapability>,
}

impl ConnectorDescriptor {
    pub fn new(
        id: ExecutionConnectorId,
        kind: ConnectorKind,
        capabilities: ConnectorCapabilities,
        recovery_capability: Option<ExecutionRecoveryCapability>,
    ) -> Result<Self, ConnectorContractError> {
        let descriptor = Self {
            id,
            kind,
            capabilities,
            recovery_capability,
        };
        descriptor.validate()?;
        Ok(descriptor)
    }

    pub fn validate(&self) -> Result<(), ConnectorContractError> {
        let required = match self.kind {
            ConnectorKind::RepositoryScript => &[ConnectorCapability::Execute][..],
            ConnectorKind::Filesystem => &[
                ConnectorCapability::ReadReceipt,
                ConnectorCapability::WriteReceipt,
                ConnectorCapability::AtomicReceipt,
            ][..],
            ConnectorKind::Transport => &[
                ConnectorCapability::PublishReceipt,
                ConnectorCapability::ConfirmDelivery,
            ][..],
        };
        let missing = required
            .iter()
            .copied()
            .filter(|capability| !self.capabilities.contains(*capability))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(ConnectorContractError::MissingCapabilities {
                connector_id: self.id.clone(),
                kind: self.kind,
                missing,
            });
        }
        match (self.kind, self.recovery_capability) {
            (
                ConnectorKind::RepositoryScript,
                Some(ExecutionRecoveryCapability::QueryableByOperationId),
            ) if !self.capabilities.contains(ConnectorCapability::Recover) => {
                Err(ConnectorContractError::MissingCapabilities {
                    connector_id: self.id.clone(),
                    kind: self.kind,
                    missing: vec![ConnectorCapability::Recover],
                })
            }
            (ConnectorKind::RepositoryScript, None) => {
                Err(ConnectorContractError::MissingRecoveryCapability {
                    connector_id: self.id.clone(),
                })
            }
            (ConnectorKind::Filesystem | ConnectorKind::Transport, Some(_)) => {
                Err(ConnectorContractError::UnexpectedRecoveryCapability {
                    connector_id: self.id.clone(),
                    kind: self.kind,
                })
            }
            _ => Ok(()),
        }
    }

    pub fn require_kind(&self, expected: ConnectorKind) -> Result<(), ConnectorContractError> {
        if self.kind != expected {
            return Err(ConnectorContractError::WrongKind {
                connector_id: self.id.clone(),
                expected,
                actual: self.kind,
            });
        }
        Ok(())
    }
    #[must_use]
    pub const fn id(&self) -> &ExecutionConnectorId {
        &self.id
    }
    #[must_use]
    pub const fn kind(&self) -> ConnectorKind {
        self.kind
    }
    #[must_use]
    pub const fn capabilities(&self) -> &ConnectorCapabilities {
        &self.capabilities
    }
    #[must_use]
    pub const fn recovery_capability(&self) -> Option<ExecutionRecoveryCapability> {
        self.recovery_capability
    }
}
