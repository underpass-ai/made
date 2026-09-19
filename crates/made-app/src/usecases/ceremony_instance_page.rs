use crate::usecases::CeremonyInstanceRead;
use crate::usecases::CeremonySearchCursor;

/// A bounded result page and the last examined id when more work remains.
#[derive(Debug, Clone)]
pub struct CeremonyInstancePage {
    reads: Vec<CeremonyInstanceRead>,
    next_cursor: Option<CeremonySearchCursor>,
}

impl CeremonyInstancePage {
    #[must_use]
    pub fn new(
        reads: Vec<CeremonyInstanceRead>,
        next_cursor: Option<CeremonySearchCursor>,
    ) -> Self {
        Self { reads, next_cursor }
    }

    #[must_use]
    pub fn reads(&self) -> &[CeremonyInstanceRead] {
        &self.reads
    }

    #[must_use]
    pub fn next_cursor(&self) -> Option<&CeremonySearchCursor> {
        self.next_cursor.as_ref()
    }
}
