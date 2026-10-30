use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{ContextKey, InputName, ProtocolName, ProtocolVersion};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolChildSpec {
    protocol: ProtocolName,
    version: ProtocolVersion,
    inputs: BTreeMap<InputName, ContextKey>,
}

impl ProtocolChildSpec {
    #[must_use]
    pub fn new(
        protocol: ProtocolName,
        version: ProtocolVersion,
        inputs: BTreeMap<InputName, ContextKey>,
    ) -> Self {
        Self {
            protocol,
            version,
            inputs,
        }
    }
    #[must_use]
    pub fn protocol(&self) -> &ProtocolName {
        &self.protocol
    }
    #[must_use]
    pub fn version(&self) -> &ProtocolVersion {
        &self.version
    }
    #[must_use]
    pub fn inputs(&self) -> &BTreeMap<InputName, ContextKey> {
        &self.inputs
    }
}
