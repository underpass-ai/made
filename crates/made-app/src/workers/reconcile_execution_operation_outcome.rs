use made_core::value_objects::{ArtifactRef, ExecutionReceipt};

/// Idempotent result of resolving an ambiguous external operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconcileExecutionOperationOutcome {
    Reconciled {
        receipt: Box<ExecutionReceipt>,
        authorization_audit: ArtifactRef,
    },
    AlreadyReconciled {
        receipt: Box<ExecutionReceipt>,
        authorization_audit: ArtifactRef,
    },
}
