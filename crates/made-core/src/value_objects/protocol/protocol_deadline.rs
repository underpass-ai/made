use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolDeadline(#[serde(with = "time::serde::rfc3339")] OffsetDateTime);
impl ProtocolDeadline {
    #[must_use]
    pub fn new(at: OffsetDateTime) -> Self {
        Self(at)
    }
    #[must_use]
    pub fn at(self) -> OffsetDateTime {
        self.0
    }
}
