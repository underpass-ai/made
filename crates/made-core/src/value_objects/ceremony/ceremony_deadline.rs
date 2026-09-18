use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyDeadline(#[serde(with = "time::serde::rfc3339")] OffsetDateTime);
impl CeremonyDeadline {
    #[must_use]
    pub fn new(at: OffsetDateTime) -> Self {
        Self(at)
    }
    #[must_use]
    pub fn at(self) -> OffsetDateTime {
        self.0
    }
}
