use made_core::value_objects::CeremonyId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnforceCeremonyDeadlinesInput {
    pub(crate) instance_id: CeremonyId,
}

impl EnforceCeremonyDeadlinesInput {
    #[must_use]
    pub fn new(instance_id: CeremonyId) -> Self {
        Self { instance_id }
    }
}
