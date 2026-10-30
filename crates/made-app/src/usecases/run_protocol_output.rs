use made_core::entities::{ProtocolDefinition, ProtocolInstance};

use super::protocol_step_trace::ProtocolStepTrace;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunProtocolOutput {
    definition: ProtocolDefinition,
    instance: ProtocolInstance,
    step_traces: Vec<ProtocolStepTrace>,
}

impl RunProtocolOutput {
    #[must_use]
    pub fn new(
        definition: ProtocolDefinition,
        instance: ProtocolInstance,
        step_traces: Vec<ProtocolStepTrace>,
    ) -> Self {
        Self {
            definition,
            instance,
            step_traces,
        }
    }

    #[must_use]
    pub fn definition(&self) -> &ProtocolDefinition {
        &self.definition
    }

    #[must_use]
    pub fn instance(&self) -> &ProtocolInstance {
        &self.instance
    }

    #[must_use]
    pub fn step_traces(&self) -> &[ProtocolStepTrace] {
        &self.step_traces
    }
}
