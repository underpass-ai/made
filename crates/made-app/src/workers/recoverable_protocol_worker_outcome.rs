use made_core::entities::ProtocolInstance;
use made_core::value_objects::{ExecutionOperationId, ExecutionReceipt};

/// Terminal outcome of one recoverable worker unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoverableProtocolWorkerOutcome {
    Completed {
        receipt: Box<ExecutionReceipt>,
        instance: Box<ProtocolInstance>,
    },
    ReconciliationRequired(ExecutionOperationId),
}
