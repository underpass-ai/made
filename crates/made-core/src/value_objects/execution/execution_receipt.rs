use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::{
    ExecutionConnectorId, ExecutionOperationId, ExecutionReceiptId, ExecutionRecoveryCapability,
    ExecutionRequestDigest, ExternalOperationId,
};
use crate::error::DomainError;
use crate::value_objects::artifact::{ArtifactRef, ArtifactSourceKind};
use crate::value_objects::ceremony::{StepClaimFence, StepResult};

pub const MAX_EXECUTION_ARTIFACTS: usize = 100;

/// Immutable terminal observation of one semantic external operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionReceipt {
    receipt_id: ExecutionReceiptId,
    operation_id: ExecutionOperationId,
    request_digest: ExecutionRequestDigest,
    producer_claim_fence: StepClaimFence,
    connector_id: ExecutionConnectorId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    external_operation_id: Option<ExternalOperationId>,
    recovery_capability: ExecutionRecoveryCapability,
    source_kind: ArtifactSourceKind,
    result: StepResult,
    artifacts: Vec<ArtifactRef>,
    #[serde(with = "time::serde::rfc3339")]
    observed_at: OffsetDateTime,
}

impl ExecutionReceipt {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        operation_id: ExecutionOperationId,
        request_digest: ExecutionRequestDigest,
        producer_claim_fence: StepClaimFence,
        connector_id: ExecutionConnectorId,
        external_operation_id: Option<ExternalOperationId>,
        recovery_capability: ExecutionRecoveryCapability,
        source_kind: ArtifactSourceKind,
        result: StepResult,
        artifacts: Vec<ArtifactRef>,
        observed_at: OffsetDateTime,
    ) -> Result<Self, DomainError> {
        if !source_kind.is_execution_source() {
            return Err(DomainError::InvariantViolated {
                reason: "execution receipt requires an execution source kind",
            });
        }
        if artifacts.len() > MAX_EXECUTION_ARTIFACTS {
            return Err(DomainError::OutOfRange {
                field: "execution_receipt.artifacts",
                value: artifacts.len() as f64,
                min: 0.0,
                max: MAX_EXECUTION_ARTIFACTS as f64,
            });
        }
        let receipt_id = ExecutionReceiptId::for_operation(&operation_id);
        for artifact in &artifacts {
            let provenance = artifact.provenance();
            if provenance.execution_receipt_id() != Some(&receipt_id)
                || provenance.operation_id() != Some(&operation_id)
                || provenance.accepted_claim_fence() != Some(&producer_claim_fence)
                || provenance.source_kind() != source_kind
            {
                return Err(DomainError::InvariantViolated {
                    reason: "execution receipt artifact provenance does not match its execution",
                });
            }
        }
        Ok(Self {
            receipt_id,
            operation_id,
            request_digest,
            producer_claim_fence,
            connector_id,
            external_operation_id,
            recovery_capability,
            source_kind,
            result,
            artifacts,
            observed_at,
        })
    }

    #[must_use]
    pub const fn receipt_id(&self) -> &ExecutionReceiptId {
        &self.receipt_id
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
    pub const fn producer_claim_fence(&self) -> &StepClaimFence {
        &self.producer_claim_fence
    }

    #[must_use]
    pub const fn connector_id(&self) -> &ExecutionConnectorId {
        &self.connector_id
    }

    #[must_use]
    pub const fn external_operation_id(&self) -> Option<&ExternalOperationId> {
        self.external_operation_id.as_ref()
    }

    #[must_use]
    pub const fn recovery_capability(&self) -> ExecutionRecoveryCapability {
        self.recovery_capability
    }

    #[must_use]
    pub const fn source_kind(&self) -> ArtifactSourceKind {
        self.source_kind
    }

    #[must_use]
    pub const fn result(&self) -> &StepResult {
        &self.result
    }

    #[must_use]
    pub fn artifacts(&self) -> &[ArtifactRef] {
        &self.artifacts
    }

    #[must_use]
    pub const fn observed_at(&self) -> OffsetDateTime {
        self.observed_at
    }
}
