use crate::usecases::ProtocolInstanceRead;
use crate::usecases::ProtocolSearchCursor;

/// A bounded result page and the last examined id when more work remains.
#[derive(Debug, Clone)]
pub struct ProtocolInstancePage {
    reads: Vec<ProtocolInstanceRead>,
    next_cursor: Option<ProtocolSearchCursor>,
}

impl ProtocolInstancePage {
    #[must_use]
    pub fn new(
        reads: Vec<ProtocolInstanceRead>,
        next_cursor: Option<ProtocolSearchCursor>,
    ) -> Self {
        Self { reads, next_cursor }
    }

    #[must_use]
    pub fn reads(&self) -> &[ProtocolInstanceRead] {
        &self.reads
    }

    #[must_use]
    pub fn next_cursor(&self) -> Option<&ProtocolSearchCursor> {
        self.next_cursor.as_ref()
    }
}
