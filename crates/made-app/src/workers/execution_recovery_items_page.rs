use made_core::value_objects::ExecutionRecoveryCursor;

use super::ExecutionRecoveryItem;

/// One bounded recovery page after applied receipt links are filtered out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionRecoveryItemsPage {
    items: Vec<ExecutionRecoveryItem>,
    next_cursor: Option<ExecutionRecoveryCursor>,
}

impl ExecutionRecoveryItemsPage {
    #[must_use]
    pub const fn new(
        items: Vec<ExecutionRecoveryItem>,
        next_cursor: Option<ExecutionRecoveryCursor>,
    ) -> Self {
        Self { items, next_cursor }
    }

    #[must_use]
    pub fn items(&self) -> &[ExecutionRecoveryItem] {
        &self.items
    }

    #[must_use]
    pub const fn next_cursor(&self) -> Option<&ExecutionRecoveryCursor> {
        self.next_cursor.as_ref()
    }

    #[must_use]
    pub fn into_parts(self) -> (Vec<ExecutionRecoveryItem>, Option<ExecutionRecoveryCursor>) {
        (self.items, self.next_cursor)
    }
}
