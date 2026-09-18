use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ChildPosition(u16);

impl ChildPosition {
    #[must_use]
    pub const fn new(value: u16) -> Self {
        Self(value)
    }
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}
