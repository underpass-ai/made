use made_core::entities::CeremonyInstance;
use made_core::value_objects::CeremonyId;

/// A bounded result page and the last examined id when more work remains.
#[derive(Debug, Clone)]
pub struct CeremonyInstancePage {
    instances: Vec<CeremonyInstance>,
    next_after: Option<CeremonyId>,
}

impl CeremonyInstancePage {
    #[must_use]
    pub fn new(instances: Vec<CeremonyInstance>, next_after: Option<CeremonyId>) -> Self {
        Self {
            instances,
            next_after,
        }
    }

    #[must_use]
    pub fn instances(&self) -> &[CeremonyInstance] {
        &self.instances
    }

    #[must_use]
    pub fn next_after(&self) -> Option<&CeremonyId> {
        self.next_after.as_ref()
    }
}
