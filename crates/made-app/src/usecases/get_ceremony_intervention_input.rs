use made_core::value_objects::{CeremonyId, CeremonyInterventionId};

/// One intervention of one ceremony, with every route it took.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetCeremonyInterventionInput {
    pub(crate) instance_id: CeremonyId,
    pub(crate) intervention_id: CeremonyInterventionId,
}

impl GetCeremonyInterventionInput {
    #[must_use]
    pub const fn new(instance_id: CeremonyId, intervention_id: CeremonyInterventionId) -> Self {
        Self {
            instance_id,
            intervention_id,
        }
    }

    #[must_use]
    pub const fn instance_id(&self) -> &CeremonyId {
        &self.instance_id
    }
}
