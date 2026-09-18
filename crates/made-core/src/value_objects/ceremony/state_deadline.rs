use super::{StateId, StateVisit};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateDeadline {
    state_id: StateId,
    state_visit: StateVisit,
    #[serde(with = "time::serde::rfc3339")]
    at: OffsetDateTime,
}
impl StateDeadline {
    #[must_use]
    pub fn new(state_id: StateId, state_visit: StateVisit, at: OffsetDateTime) -> Self {
        Self {
            state_id,
            state_visit,
            at,
        }
    }
    #[must_use]
    pub fn state_id(&self) -> &StateId {
        &self.state_id
    }
    #[must_use]
    pub fn state_visit(&self) -> StateVisit {
        self.state_visit
    }
    #[must_use]
    pub fn at(&self) -> OffsetDateTime {
        self.at
    }
}
