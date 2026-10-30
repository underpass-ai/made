use super::{ReceiptStatus, ScriptObservation};
use crate::connectors::ConnectorError;
use made_core::value_objects::{
    ArtifactSourceKind, ExecutionConnectorId, ExecutionIntent, ExecutionOperationId,
    ExecutionRecoveryCapability, ExecutionRequestDigest, ExternalOperationId, StepClaimFence,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

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
        observation: &ScriptObservation,
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
