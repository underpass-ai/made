use made_core::value_objects::{AuthorizedOperation, ExecutionReceipt};

/// Operator-backed terminal observation for one previously persisted intent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconcileExecutionOperationInput {
    pub authorization: AuthorizedOperation,
    pub receipt: ExecutionReceipt,
}
