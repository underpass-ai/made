use std::collections::BTreeSet;
use std::fmt;

use made_core::value_objects::{ExecutionConnectorId, ExecutionRecoveryCapability};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Kind of external boundary represented by a connector descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorKind {
    RepositoryScript,
    Filesystem,
    Transport,
}

/// Capabilities are explicit so a composition fails at construction time,
/// rather than at the first recovery or delivery attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorCapability {
    Execute,
    Recover,
    ReadReceipt,
    WriteReceipt,
    AtomicReceipt,
    PublishReceipt,
    ConfirmDelivery,
}

/// Set of capabilities advertised by one connector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorCapabilities(BTreeSet<ConnectorCapability>);

impl ConnectorCapabilities {
    #[must_use]
    pub fn new(capabilities: impl IntoIterator<Item = ConnectorCapability>) -> Self {
        Self(capabilities.into_iter().collect())
    }

    #[must_use]
    pub fn empty() -> Self {
        Self(BTreeSet::new())
    }

    #[must_use]
    pub fn contains(&self, capability: ConnectorCapability) -> bool {
        self.0.contains(&capability)
    }

    pub fn iter(&self) -> impl Iterator<Item = ConnectorCapability> + '_ {
        self.0.iter().copied()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Safe, stable identity and capability declaration for an adapter.
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

/// Errors returned while checking an integration composition.
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

impl fmt::Display for ConnectorCapabilities {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_list().entries(self.0.iter()).finish()
    }
}
