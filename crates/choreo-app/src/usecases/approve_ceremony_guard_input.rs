use choreo_core::value_objects::{CeremonyId, GuardName};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApproveCeremonyGuardInput {
    pub(crate) instance_id: CeremonyId,
    pub(crate) guard_name: GuardName,
}

impl ApproveCeremonyGuardInput {
    #[must_use]
    pub fn new(instance_id: CeremonyId, guard_name: GuardName) -> Self {
        Self {
            instance_id,
            guard_name,
        }
    }
}
