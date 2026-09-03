/// Result of an idempotent durable-memory write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryWriteOutcome {
    Remembered,
    AlreadyRemembered,
    NotRemembered,
}
