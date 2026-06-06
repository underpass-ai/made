use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputRequirement {
    Required,
    Optional,
}

impl InputRequirement {
    #[must_use]
    pub fn is_required(self) -> bool {
        self == Self::Required
    }
}
