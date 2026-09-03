use crate::engine::detect::StorageEngine;

/// What a conversion moved, for an operator who has to believe it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConversionReceipt {
    pub source_engine: StorageEngine,
    pub destination_engine: StorageEngine,
    pub ceremonies: u64,
    pub journal_records: u64,
    pub outbox_messages: u64,
    pub publications: u64,
}
