use serde::{Deserialize, Serialize};

use super::OutputName;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolOutputDefinition {
    name: OutputName,
}

impl ProtocolOutputDefinition {
    #[must_use]
    pub fn new(name: OutputName) -> Self {
        Self { name }
    }

    #[must_use]
    pub fn name(&self) -> &OutputName {
        &self.name
    }
}
