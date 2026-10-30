use serde::{Deserialize, Serialize};

use super::StateId;

/// Reference to the repeating state whose iteration cap may route one
/// transition.
///
/// The state-level counterpart of
/// [`StepRepeatExhaustedGuardCondition`](super::StepRepeatExhaustedGuardCondition):
/// it holds once every step of the named state finished its last
/// permitted iteration without the state's `until` condition becoming
/// true, and only on a transition leaving that state. Such a transition
/// is the one move an exhausted state repeat still permits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateRepeatExhaustedGuardCondition {
    state_id: StateId,
}

impl StateRepeatExhaustedGuardCondition {
    #[must_use]
    pub fn new(state_id: StateId) -> Self {
        Self { state_id }
    }

    #[must_use]
    pub fn state_id(&self) -> &StateId {
        &self.state_id
    }
}
