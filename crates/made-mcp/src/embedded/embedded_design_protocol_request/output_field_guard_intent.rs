use made_app::usecases::ProtocolDesignOutputFieldGuard;
use made_core::error::DomainError;
use made_core::value_objects::{StepId, StepOutputField};
use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OutputFieldGuardIntent {
    step: String,
    output_field: String,
    equals: Value,
}

impl OutputFieldGuardIntent {
    pub(super) fn into_domain(self) -> Result<ProtocolDesignOutputFieldGuard, DomainError> {
        Ok(ProtocolDesignOutputFieldGuard::new(
            StepId::new(self.step)?,
            StepOutputField::new(self.output_field)?,
            self.equals,
        ))
    }
}
