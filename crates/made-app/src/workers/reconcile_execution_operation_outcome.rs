use made_core::value_objects::ExecutionReceipt;

/// Idempotent result of resolving an ambiguous external operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconcileExecutionOperationOutcome {
    Reconciled(Box<ExecutionReceipt>),
    AlreadyReconciled(Box<ExecutionReceipt>),
}
