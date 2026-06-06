use serde::{Deserialize, Serialize};

use super::{InputName, InputRequirement};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyInputDefinition {
    name: InputName,
    requirement: InputRequirement,
}

impl CeremonyInputDefinition {
    #[must_use]
    pub fn new(name: InputName, requirement: InputRequirement) -> Self {
        Self { name, requirement }
    }

    #[must_use]
    pub fn required(name: InputName) -> Self {
        Self::new(name, InputRequirement::Required)
    }

    #[must_use]
    pub fn optional(name: InputName) -> Self {
        Self::new(name, InputRequirement::Optional)
    }

    #[must_use]
    pub fn name(&self) -> &InputName {
        &self.name
    }

    #[must_use]
    pub fn requirement(&self) -> InputRequirement {
        self.requirement
    }
}
