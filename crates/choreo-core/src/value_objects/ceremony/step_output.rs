use serde::{Deserialize, Serialize};

use crate::value_objects::Attributes;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StepOutput(Attributes);

impl StepOutput {
    #[must_use]
    pub fn new(attributes: Attributes) -> Self {
        Self(attributes)
    }

    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn attributes(&self) -> &Attributes {
        &self.0
    }
}
