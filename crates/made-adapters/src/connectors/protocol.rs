#![allow(clippy::needless_pass_by_value)]

use std::fmt;

use async_trait::async_trait;
use made_core::value_objects::{
    ArtifactSourceKind, ExecutionConnectorId, ExecutionIntent, ExecutionOperationId,
    ExecutionRecoveryCapability, ExecutionRequestDigest, ExternalOperationId, StepClaimFence,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::{ConnectorDescriptor, ConnectorError};

/// Inputs passed to a repository script.  Its `Debug` implementation omits
/// request bytes because they may contain user-authored secrets.
#[derive(Clone, PartialEq, Eq)]
pub struct ScriptInvocation {
    operation_id: ExecutionOperationId,
    request_digest: ExecutionRequestDigest,
    claim_fence: StepClaimFence,
    request_bytes: Vec<u8>,
}

impl ScriptInvocation {
    #[must_use]
    pub fn from_intent(intent: &ExecutionIntent) -> Self {
        Self {
            operation_id: intent.operation().operation_id().clone(),
            request_digest: intent.operation().request_digest().clone(),
            claim_fence: intent.claim_fence().clone(),
            request_bytes: intent.operation().request().as_bytes().to_vec(),
        }
    }

    #[must_use]
    pub const fn operation_id(&self) -> &ExecutionOperationId {
        &self.operation_id
    }

    #[must_use]
    pub const fn request_digest(&self) -> &ExecutionRequestDigest {
        &self.request_digest
    }

    #[must_use]
    pub const fn claim_fence(&self) -> &StepClaimFence {
        &self.claim_fence
    }

    #[must_use]
    pub fn request_bytes(&self) -> &[u8] {
        &self.request_bytes
    }
}

impl fmt::Debug for ScriptInvocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ScriptInvocation")
            .field("operation_id", &self.operation_id)
            .field("request_digest", &self.request_digest)
            .field("claim_fence", &self.claim_fence)
            .field("request_byte_count", &self.request_bytes.len())
            .finish()
    }
}

/// Public status suitable for persistence and transport.  It carries no
/// process output or backend error text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptStatus {
    Completed,
    Failed,
}

/// Terminal observation supplied by a repository/script connector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptObservation {
    producer_claim_fence: StepClaimFence,
    external_operation_id: Option<ExternalOperationId>,
    status: ReceiptStatus,
    stdout_bytes: u64,
    stderr_bytes: u64,
    observed_at: OffsetDateTime,
}

impl ScriptObservation {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        producer_claim_fence: StepClaimFence,
        external_operation_id: Option<ExternalOperationId>,
        status: ReceiptStatus,
        stdout_bytes: u64,
        stderr_bytes: u64,
        observed_at: OffsetDateTime,
    ) -> Self {
        Self {
            producer_claim_fence,
            external_operation_id,
            status,
            stdout_bytes,
            stderr_bytes,
            observed_at,
        }
    }

    #[must_use]
    pub const fn producer_claim_fence(&self) -> &StepClaimFence {
        &self.producer_claim_fence
    }

    #[must_use]
    pub const fn external_operation_id(&self) -> Option<&ExternalOperationId> {
        self.external_operation_id.as_ref()
    }

    #[must_use]
    pub const fn status(&self) -> ReceiptStatus {
        self.status
    }

    #[must_use]
    pub const fn stdout_bytes(&self) -> u64 {
        self.stdout_bytes
    }

    #[must_use]
    pub const fn stderr_bytes(&self) -> u64 {
        self.stderr_bytes
    }

    #[must_use]
    pub const fn observed_at(&self) -> OffsetDateTime {
        self.observed_at
    }
}

/// Result of an execution attempt.  Reconciliation is explicit and is never
/// represented as a successful receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptResolution {
    Observed(ScriptObservation),
    ReconciliationRequired,
}

/// Receipt deliberately restricted to durable, non-secret evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SafeExecutionReceipt {
    operation_id: ExecutionOperationId,
    request_digest: ExecutionRequestDigest,
    producer_claim_fence: StepClaimFence,
    connector_id: ExecutionConnectorId,
    recovery_capability: ExecutionRecoveryCapability,
    source_kind: ArtifactSourceKind,
    external_operation_id: Option<ExternalOperationId>,
    status: ReceiptStatus,
    stdout_bytes: u64,
    stderr_bytes: u64,
    #[serde(with = "time::serde::rfc3339")]
    observed_at: OffsetDateTime,
}

impl SafeExecutionReceipt {
    pub(crate) fn from_observation(
        intent: &ExecutionIntent,
        connector_id: ExecutionConnectorId,
        recovery_capability: ExecutionRecoveryCapability,
        observation: ScriptObservation,
    ) -> Result<Self, ConnectorError> {
        if observation.producer_claim_fence() != intent.claim_fence() {
            return Err(ConnectorError::ReceiptMismatch {
                field: "producer_claim_fence",
            });
        }
        observation
            .stdout_bytes()
            .checked_add(observation.stderr_bytes())
            .ok_or(ConnectorError::ReceiptMismatch {
                field: "output_bytes",
            })?;
        Ok(Self {
            operation_id: intent.operation().operation_id().clone(),
            request_digest: intent.operation().request_digest().clone(),
            producer_claim_fence: observation.producer_claim_fence().clone(),
            connector_id,
            recovery_capability,
            source_kind: intent.source_kind(),
            external_operation_id: observation.external_operation_id().cloned(),
            status: observation.status(),
            stdout_bytes: observation.stdout_bytes(),
            stderr_bytes: observation.stderr_bytes(),
            observed_at: observation.observed_at(),
        })
    }

    pub(crate) fn validate_for(
        &self,
        intent: &ExecutionIntent,
        connector_id: &ExecutionConnectorId,
        recovery_capability: ExecutionRecoveryCapability,
    ) -> Result<(), ConnectorError> {
        if &self.operation_id != intent.operation().operation_id() {
            return Err(ConnectorError::ReceiptMismatch {
                field: "operation_id",
            });
        }
        if &self.request_digest != intent.operation().request_digest() {
            return Err(ConnectorError::ReceiptMismatch {
                field: "request_digest",
            });
        }
        if &self.producer_claim_fence != intent.claim_fence() {
            return Err(ConnectorError::ReceiptMismatch {
                field: "producer_claim_fence",
            });
        }
        if &self.connector_id != connector_id {
            return Err(ConnectorError::ReceiptMismatch {
                field: "connector_id",
            });
        }
        if self.recovery_capability != recovery_capability {
            return Err(ConnectorError::ReceiptMismatch {
                field: "recovery_capability",
            });
        }
        if self.source_kind != intent.source_kind() {
            return Err(ConnectorError::ReceiptMismatch {
                field: "source_kind",
            });
        }
        self.stdout_bytes.checked_add(self.stderr_bytes).ok_or(
            ConnectorError::ReceiptMismatch {
                field: "output_bytes",
            },
        )?;
        Ok(())
    }

    #[must_use]
    pub const fn operation_id(&self) -> &ExecutionOperationId {
        &self.operation_id
    }

    #[must_use]
    pub const fn request_digest(&self) -> &ExecutionRequestDigest {
        &self.request_digest
    }

    #[must_use]
    pub const fn connector_id(&self) -> &ExecutionConnectorId {
        &self.connector_id
    }

    #[must_use]
    pub const fn status(&self) -> ReceiptStatus {
        self.status
    }

    #[must_use]
    pub const fn stdout_bytes(&self) -> u64 {
        self.stdout_bytes
    }

    #[must_use]
    pub const fn stderr_bytes(&self) -> u64 {
        self.stderr_bytes
    }

    #[must_use]
    pub const fn observed_at(&self) -> OffsetDateTime {
        self.observed_at
    }
}

/// Repository/script boundary.  Implementations may use the existing
/// durable script protocol; this contract only exposes safe terminal data.
#[async_trait]
pub trait RepositoryScriptConnector: Send + Sync {
    fn descriptor(&self) -> &ConnectorDescriptor;

    async fn execute_or_recover(
        &self,
        invocation: ScriptInvocation,
    ) -> Result<ScriptResolution, ConnectorError>;
}

/// Durable filesystem boundary for safe receipts.
#[async_trait]
pub trait ReceiptFilesystemConnector: Send + Sync {
    fn descriptor(&self) -> &ConnectorDescriptor;

    async fn load(
        &self,
        operation_id: &ExecutionOperationId,
    ) -> Result<Option<SafeExecutionReceipt>, ConnectorError>;

    async fn store(&self, receipt: &SafeExecutionReceipt) -> Result<(), ConnectorError>;
}

/// Transport boundary.  Implementations must make publication idempotent by
/// operation ID or tolerate a retry after a persisted receipt.
#[async_trait]
pub trait ReceiptTransportConnector: Send + Sync {
    fn descriptor(&self) -> &ConnectorDescriptor;

    async fn publish(&self, receipt: &SafeExecutionReceipt) -> Result<(), ConnectorError>;
}
