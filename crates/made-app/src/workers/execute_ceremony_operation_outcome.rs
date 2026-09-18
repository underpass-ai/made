use made_core::value_objects::{ExecutionOperationId, ExecutionReceipt};

/// Result of a fresh worker attempt after its intent is durable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecuteCeremonyOperationOutcome {
    Receipt(Box<ExecutionReceipt>),
    ReconciliationRequired(ExecutionOperationId),
}
