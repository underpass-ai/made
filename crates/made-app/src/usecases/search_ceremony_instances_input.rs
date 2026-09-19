use made_core::value_objects::{
    CeremonyIdPrefix, CeremonyInstancePageLimit, CeremonyLifecyclePhase,
};

use crate::usecases::CeremonySearchCursor;

/// Validated application query behind the public paginated listing.
#[derive(Debug, Clone)]
pub struct SearchCeremonyInstancesInput {
    cursor: Option<CeremonySearchCursor>,
    limit: CeremonyInstancePageLimit,
    id_prefix: Option<CeremonyIdPrefix>,
    lifecycle: Option<CeremonyLifecyclePhase>,
}

impl SearchCeremonyInstancesInput {
    #[must_use]
    pub fn new(
        cursor: Option<CeremonySearchCursor>,
        limit: CeremonyInstancePageLimit,
        id_prefix: Option<CeremonyIdPrefix>,
        lifecycle: Option<CeremonyLifecyclePhase>,
    ) -> Self {
        Self {
            cursor,
            limit,
            id_prefix,
            lifecycle,
        }
    }

    #[must_use]
    pub fn cursor(&self) -> Option<&CeremonySearchCursor> {
        self.cursor.as_ref()
    }

    #[must_use]
    pub const fn limit(&self) -> CeremonyInstancePageLimit {
        self.limit
    }

    #[must_use]
    pub fn id_prefix(&self) -> Option<&CeremonyIdPrefix> {
        self.id_prefix.as_ref()
    }

    #[must_use]
    pub const fn lifecycle(&self) -> Option<CeremonyLifecyclePhase> {
        self.lifecycle
    }
}
