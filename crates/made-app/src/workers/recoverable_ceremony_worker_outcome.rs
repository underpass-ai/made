use made_core::entities::CeremonyInstance;
use made_core::value_objects::{ExecutionOperationId, ExecutionReceipt};

/// Terminal outcome of one recoverable worker unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoverableCeremonyWorkerOutcome {
    Completed {
        receipt: Box<ExecutionReceipt>,
        instance: Box<CeremonyInstance>,
    },
    ReconciliationRequired(ExecutionOperationId),
}
