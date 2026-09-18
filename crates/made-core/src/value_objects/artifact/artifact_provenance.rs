use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::{ArtifactImportRef, ArtifactSourceKind};
use crate::error::DomainError;
use crate::value_objects::ceremony::StepClaimFence;
use crate::value_objects::execution::{ExecutionOperationId, ExecutionReceiptId};

/// Typed origin of artifact metadata. Authentication joins in C5.7.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactProvenance {
    source_kind: ArtifactSourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    execution_receipt_id: Option<ExecutionReceiptId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    operation_id: Option<ExecutionOperationId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    accepted_claim_fence: Option<StepClaimFence>,
    #[serde(with = "time::serde::rfc3339")]
    observed_at: OffsetDateTime,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    import_ref: Option<ArtifactImportRef>,
}

impl ArtifactProvenance {
    pub fn execution(
        source_kind: ArtifactSourceKind,
        execution_receipt_id: ExecutionReceiptId,
        operation_id: ExecutionOperationId,
        accepted_claim_fence: StepClaimFence,
        observed_at: OffsetDateTime,
    ) -> Result<Self, DomainError> {
        if !source_kind.is_execution_source() {
            return Err(DomainError::InvariantViolated {
                reason: "execution artifact provenance requires an execution source kind",
            });
        }
        Ok(Self {
            source_kind,
            execution_receipt_id: Some(execution_receipt_id),
            operation_id: Some(operation_id),
            accepted_claim_fence: Some(accepted_claim_fence),
            observed_at,
            import_ref: None,
        })
    }

    #[must_use]
    pub fn generated_report(observed_at: OffsetDateTime) -> Self {
        Self {
            source_kind: ArtifactSourceKind::GeneratedReport,
            execution_receipt_id: None,
            operation_id: None,
            accepted_claim_fence: None,
            observed_at,
            import_ref: None,
        }
    }

    #[must_use]
    pub fn imported(import_ref: ArtifactImportRef, observed_at: OffsetDateTime) -> Self {
        Self {
            source_kind: ArtifactSourceKind::Imported,
            execution_receipt_id: None,
            operation_id: None,
            accepted_claim_fence: None,
            observed_at,
            import_ref: Some(import_ref),
        }
    }

    #[must_use]
    pub const fn source_kind(&self) -> ArtifactSourceKind {
        self.source_kind
    }

    #[must_use]
    pub fn execution_receipt_id(&self) -> Option<&ExecutionReceiptId> {
        self.execution_receipt_id.as_ref()
    }

    #[must_use]
    pub fn operation_id(&self) -> Option<&ExecutionOperationId> {
        self.operation_id.as_ref()
    }

    #[must_use]
    pub fn accepted_claim_fence(&self) -> Option<&StepClaimFence> {
        self.accepted_claim_fence.as_ref()
    }

    #[must_use]
    pub const fn observed_at(&self) -> OffsetDateTime {
        self.observed_at
    }

    #[must_use]
    pub fn import_ref(&self) -> Option<&ArtifactImportRef> {
        self.import_ref.as_ref()
    }
}
