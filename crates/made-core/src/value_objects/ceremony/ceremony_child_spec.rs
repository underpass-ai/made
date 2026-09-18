use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{CeremonyName, CeremonyVersion, ContextKey, InputName};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyChildSpec {
    ceremony: CeremonyName,
    version: CeremonyVersion,
    inputs: BTreeMap<InputName, ContextKey>,
}

impl CeremonyChildSpec {
    #[must_use]
    pub fn new(
        ceremony: CeremonyName,
        version: CeremonyVersion,
        inputs: BTreeMap<InputName, ContextKey>,
    ) -> Self {
        Self {
            ceremony,
            version,
            inputs,
        }
    }
    #[must_use]
    pub fn ceremony(&self) -> &CeremonyName {
        &self.ceremony
    }
    #[must_use]
    pub fn version(&self) -> &CeremonyVersion {
        &self.version
    }
    #[must_use]
    pub fn inputs(&self) -> &BTreeMap<InputName, ContextKey> {
        &self.inputs
    }
}
