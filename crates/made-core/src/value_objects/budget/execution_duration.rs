use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ExecutionDuration(u64);
impl ExecutionDuration {
    #[must_use]
    pub const fn from_micros(value: u64) -> Self {
        Self(value)
    }
    #[must_use]
    pub const fn as_micros(self) -> u64 {
        self.0
    }
}
