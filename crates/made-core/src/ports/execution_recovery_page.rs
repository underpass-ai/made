use crate::value_objects::{ExecutionOperation, ExecutionRecoveryCursor};

/// One bounded keyset page of operation roots, including roots with receipts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionRecoveryPage {
    operations: Vec<ExecutionOperation>,
    next_cursor: Option<ExecutionRecoveryCursor>,
}

impl ExecutionRecoveryPage {
    #[must_use]
    pub const fn new(
        operations: Vec<ExecutionOperation>,
        next_cursor: Option<ExecutionRecoveryCursor>,
    ) -> Self {
        Self {
            operations,
            next_cursor,
        }
    }

    #[must_use]
    pub fn operations(&self) -> &[ExecutionOperation] {
        &self.operations
    }

    #[must_use]
    pub const fn next_cursor(&self) -> Option<&ExecutionRecoveryCursor> {
        self.next_cursor.as_ref()
    }

    #[must_use]
    pub fn into_parts(self) -> (Vec<ExecutionOperation>, Option<ExecutionRecoveryCursor>) {
        (self.operations, self.next_cursor)
    }
}
