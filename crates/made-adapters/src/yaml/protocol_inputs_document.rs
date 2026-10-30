use made_core::error::DomainError;
use made_core::value_objects::{InputName, ProtocolInputDefinition};
use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct ProtocolInputsDocument {
    #[serde(default)]
    required: Vec<String>,
    #[serde(default)]
    optional: Vec<String>,
}

impl ProtocolInputsDocument {
    pub(super) fn into_domain(self) -> Result<Vec<ProtocolInputDefinition>, DomainError> {
        let mut inputs = Vec::with_capacity(self.required.len() + self.optional.len());
        for name in self.required {
            inputs.push(ProtocolInputDefinition::required(InputName::new(name)?));
        }
        for name in self.optional {
            inputs.push(ProtocolInputDefinition::optional(InputName::new(name)?));
        }
        Ok(inputs)
    }
}
