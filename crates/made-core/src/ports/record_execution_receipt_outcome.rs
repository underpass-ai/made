/// Idempotent result of inserting an immutable terminal receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordExecutionReceiptOutcome {
    Recorded,
    AlreadyRecorded,
}
