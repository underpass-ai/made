use std::collections::BTreeMap;

use made_core::error::DomainError;
use made_core::value_objects::{
    CeremonyChildSpec, CeremonyName, CeremonyVersion, ContextKey, InputName,
};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ChildSpecIntent {
    ceremony: String,
    version: String,
    #[serde(default)]
    inputs: BTreeMap<String, String>,
}

impl ChildSpecIntent {
    pub(super) fn into_domain(self) -> Result<CeremonyChildSpec, DomainError> {
        let inputs = self
            .inputs
            .into_iter()
            .map(|(input, context)| Ok((InputName::new(input)?, ContextKey::new(context)?)))
            .collect::<Result<BTreeMap<_, _>, DomainError>>()?;
        Ok(CeremonyChildSpec::new(
            CeremonyName::new(self.ceremony)?,
            CeremonyVersion::new(self.version)?,
            inputs,
        ))
    }
}
