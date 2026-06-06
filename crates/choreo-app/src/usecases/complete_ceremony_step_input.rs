use choreo_core::value_objects::{CeremonyId, CeremonyName, CeremonyVersion, StepId, StepResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteCeremonyStepInput {
    pub(crate) instance_id: CeremonyId,
    pub(crate) definition_name: CeremonyName,
    pub(crate) definition_version: CeremonyVersion,
    pub(crate) step_id: StepId,
    pub(crate) result: StepResult,
}

impl CompleteCeremonyStepInput {
    #[must_use]
    pub fn new(
        instance_id: CeremonyId,
        definition_name: CeremonyName,
        definition_version: CeremonyVersion,
        step_id: StepId,
        result: StepResult,
    ) -> Self {
        Self {
            instance_id,
            definition_name,
            definition_version,
            step_id,
            result,
        }
    }
}
