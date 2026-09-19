use crate::usecases::CeremonySearchCursor;
use made_core::entities::CeremonyInstance;

/// A bounded result page and the last examined id when more work remains.
#[derive(Debug, Clone)]
pub struct CeremonyInstancePage {
    instances: Vec<CeremonyInstance>,
    next_cursor: Option<CeremonySearchCursor>,
}

impl CeremonyInstancePage {
    #[must_use]
    pub fn new(
        instances: Vec<CeremonyInstance>,
        next_cursor: Option<CeremonySearchCursor>,
    ) -> Self {
        Self {
            instances,
            next_cursor,
        }
    }

    #[must_use]
    pub fn instances(&self) -> &[CeremonyInstance] {
        &self.instances
    }

    #[must_use]
    pub fn next_cursor(&self) -> Option<&CeremonySearchCursor> {
        self.next_cursor.as_ref()
    }
}
