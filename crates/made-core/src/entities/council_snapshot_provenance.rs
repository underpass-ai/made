use crate::value_objects::CouncilSnapshotSource;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// An observed prior data snapshot, with no claims about the events before it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CouncilSnapshotProvenance {
    source: CouncilSnapshotSource,
    captured_at: OffsetDateTime,
}
impl CouncilSnapshotProvenance {
    #[must_use]
    pub fn new(source: CouncilSnapshotSource, captured_at: OffsetDateTime) -> Self {
        Self {
            source,
            captured_at,
        }
    }
    #[must_use]
    pub fn source(&self) -> &CouncilSnapshotSource {
        &self.source
    }
    #[must_use]
    pub fn captured_at(&self) -> OffsetDateTime {
        self.captured_at
    }
}
