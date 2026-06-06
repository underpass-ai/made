use choreo_core::error::DomainError;
use choreo_core::value_objects::{CeremonyInputDefinition, InputName};
use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct CeremonyInputsDocument {
    #[serde(default)]
    required: Vec<String>,
    #[serde(default)]
    optional: Vec<String>,
}

impl CeremonyInputsDocument {
    pub(super) fn into_domain(self) -> Result<Vec<CeremonyInputDefinition>, DomainError> {
        let mut inputs = Vec::with_capacity(self.required.len() + self.optional.len());
        for name in self.required {
            inputs.push(CeremonyInputDefinition::required(InputName::new(name)?));
        }
        for name in self.optional {
            inputs.push(CeremonyInputDefinition::optional(InputName::new(name)?));
        }
        Ok(inputs)
    }
}
