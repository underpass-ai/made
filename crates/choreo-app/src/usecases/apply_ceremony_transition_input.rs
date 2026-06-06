use choreo_core::value_objects::{
    CeremonyId, CeremonyName, CeremonyVersion, RoleId, TransitionTrigger,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyCeremonyTransitionInput {
    pub(crate) instance_id: CeremonyId,
    pub(crate) definition_name: CeremonyName,
    pub(crate) definition_version: CeremonyVersion,
    pub(crate) role_id: RoleId,
    pub(crate) trigger: TransitionTrigger,
}

impl ApplyCeremonyTransitionInput {
    #[must_use]
    pub fn new(
        instance_id: CeremonyId,
        definition_name: CeremonyName,
        definition_version: CeremonyVersion,
        role_id: RoleId,
        trigger: TransitionTrigger,
    ) -> Self {
        Self {
            instance_id,
            definition_name,
            definition_version,
            role_id,
            trigger,
        }
    }
}
