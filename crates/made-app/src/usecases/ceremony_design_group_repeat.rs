use made_core::value_objects::StateIteration;

use super::CeremonyDesignGroupRepeatUntil;

#[derive(Debug, Clone, PartialEq)]
pub struct CeremonyDesignGroupRepeat {
    max_iterations: StateIteration,
    until: CeremonyDesignGroupRepeatUntil,
}

impl CeremonyDesignGroupRepeat {
    #[must_use]
    pub fn new(max_iterations: StateIteration, until: CeremonyDesignGroupRepeatUntil) -> Self {
        Self {
            max_iterations,
            until,
        }
    }
    #[must_use]
    pub const fn max_iterations(&self) -> StateIteration {
        self.max_iterations
    }
    #[must_use]
    pub const fn until(&self) -> &CeremonyDesignGroupRepeatUntil {
        &self.until
    }
}
