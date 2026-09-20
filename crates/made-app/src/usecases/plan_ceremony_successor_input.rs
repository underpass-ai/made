use made_core::value_objects::{CeremonyId, CeremonyName, CeremonyVersion};

/// Which ceremony would hand off, and to which published definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanCeremonySuccessorInput {
    pub instance_id: CeremonyId,
    pub definition_name: CeremonyName,
    pub definition_version: CeremonyVersion,
}
