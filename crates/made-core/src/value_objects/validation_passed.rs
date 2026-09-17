use serde::{Deserialize, Serialize};

/// Whether the selected council decision passed its requested validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ValidationPassed(bool);

impl ValidationPassed {
    #[must_use]
    pub const fn new(passed: bool) -> Self {
        Self(passed)
    }

    #[must_use]
    pub const fn get(self) -> bool {
        self.0
    }
}
