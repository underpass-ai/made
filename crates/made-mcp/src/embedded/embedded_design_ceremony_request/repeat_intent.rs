use made_app::usecases::CeremonyDesignRepeat;
use made_core::error::DomainError;
use made_core::value_objects::{StepIteration, StepOutputField};
use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RepeatIntent {
    max_iterations: u32,
    output_field: String,
    equals: Value,
}

impl RepeatIntent {
    pub(super) fn into_domain(self) -> Result<CeremonyDesignRepeat, DomainError> {
        Ok(CeremonyDesignRepeat::new(
            StepIteration::new(self.max_iterations)?,
            StepOutputField::new(self.output_field)?,
            self.equals,
        ))
    }
}
