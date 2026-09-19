use made_core::value_objects::{
    CeremonyId, CeremonyIdPrefix, CeremonyInstancePageLimit, CeremonyLifecyclePhase,
};

/// Validated application query behind the public paginated listing.
#[derive(Debug, Clone)]
pub struct SearchCeremonyInstancesInput {
    after: Option<CeremonyId>,
    limit: CeremonyInstancePageLimit,
    id_prefix: Option<CeremonyIdPrefix>,
    lifecycle: Option<CeremonyLifecyclePhase>,
}

impl SearchCeremonyInstancesInput {
    #[must_use]
    pub fn new(
        after: Option<CeremonyId>,
        limit: CeremonyInstancePageLimit,
        id_prefix: Option<CeremonyIdPrefix>,
        lifecycle: Option<CeremonyLifecyclePhase>,
    ) -> Self {
        Self {
            after,
            limit,
            id_prefix,
            lifecycle,
        }
    }

    #[must_use]
    pub fn after(&self) -> Option<&CeremonyId> {
        self.after.as_ref()
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
