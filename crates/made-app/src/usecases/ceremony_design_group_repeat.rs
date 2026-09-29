use made_core::value_objects::{StateIteration, StepId};

use super::CeremonyDesignGroupRepeatUntil;

#[derive(Debug, Clone, PartialEq)]
pub struct CeremonyDesignGroupRepeat {
    max_iterations: StateIteration,
    until: CeremonyDesignGroupRepeatUntil,
    exhausted_terminal: Option<StepId>,
}

impl CeremonyDesignGroupRepeat {
    #[must_use]
    pub fn new(max_iterations: StateIteration, until: CeremonyDesignGroupRepeatUntil) -> Self {
        Self {
            max_iterations,
            until,
            exhausted_terminal: None,
        }
    }

    /// Route the group's exhaustion — its last permitted iteration ends
    /// without `until` holding — to a terminal state of this identity
    /// instead of leaving the ceremony stopped in the group.
    #[must_use]
    pub fn with_exhausted_terminal(mut self, terminal: StepId) -> Self {
        self.exhausted_terminal = Some(terminal);
        self
    }
    #[must_use]
    pub const fn max_iterations(&self) -> StateIteration {
        self.max_iterations
    }
    #[must_use]
    pub const fn until(&self) -> &CeremonyDesignGroupRepeatUntil {
        &self.until
    }
    #[must_use]
    pub fn exhausted_terminal(&self) -> Option<&StepId> {
        self.exhausted_terminal.as_ref()
    }
}
