use made_core::value_objects::{ExecutionOperationId, ExecutionReceipt};

/// Result of settling one durable intent after process restart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoverExecutionIntentOutcome {
    Receipt(Box<ExecutionReceipt>),
    ReconciliationRequired(ExecutionOperationId),
}
