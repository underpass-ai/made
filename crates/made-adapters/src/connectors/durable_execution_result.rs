use super::SafeExecutionReceipt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DurableExecutionResult {
    Receipt(SafeExecutionReceipt),
    ReconciliationRequired,
}
