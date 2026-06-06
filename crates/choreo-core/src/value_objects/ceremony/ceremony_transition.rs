use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

use super::{GuardName, StateId, TransitionTrigger};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyTransition {
    from: StateId,
    to: StateId,
    trigger: TransitionTrigger,
    required_guards: BTreeSet<GuardName>,
}

impl CeremonyTransition {
    pub fn new(
        from: StateId,
        to: StateId,
        trigger: TransitionTrigger,
        required_guards: impl IntoIterator<Item = GuardName>,
    ) -> Result<Self, DomainError> {
        let required_guards = required_guards.into_iter().collect();
        Ok(Self {
            from,
            to,
            trigger,
            required_guards,
        })
    }

    #[must_use]
    pub fn from(&self) -> &StateId {
        &self.from
    }

    #[must_use]
    pub fn to(&self) -> &StateId {
        &self.to
    }

    #[must_use]
    pub fn trigger(&self) -> &TransitionTrigger {
        &self.trigger
    }

    #[must_use]
    pub fn required_guards(&self) -> &BTreeSet<GuardName> {
        &self.required_guards
    }
}
