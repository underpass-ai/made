use choreo_core::entities::{CeremonyDefinition, CeremonyInstance};

use super::ceremony_step_trace::CeremonyStepTrace;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunCeremonyOutput {
    definition: CeremonyDefinition,
    instance: CeremonyInstance,
    step_traces: Vec<CeremonyStepTrace>,
}

impl RunCeremonyOutput {
    #[must_use]
    pub fn new(
        definition: CeremonyDefinition,
        instance: CeremonyInstance,
        step_traces: Vec<CeremonyStepTrace>,
    ) -> Self {
        Self {
            definition,
            instance,
            step_traces,
        }
    }

    #[must_use]
    pub fn definition(&self) -> &CeremonyDefinition {
        &self.definition
    }

    #[must_use]
    pub fn instance(&self) -> &CeremonyInstance {
        &self.instance
    }

    #[must_use]
    pub fn step_traces(&self) -> &[CeremonyStepTrace] {
        &self.step_traces
    }
}
