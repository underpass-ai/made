use made_core::error::DomainError;
use made_core::value_objects::{CeremonyTransition, GuardName, StateId, TransitionTrigger};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub(super) struct CeremonyTransitionDocument {
    from: String,
    to: String,
    trigger: String,
    #[serde(default)]
    guards: Vec<String>,
}

impl CeremonyTransitionDocument {
    pub(super) fn into_domain(self) -> Result<CeremonyTransition, DomainError> {
        CeremonyTransition::new(
            StateId::new(self.from)?,
            StateId::new(self.to)?,
            TransitionTrigger::new(self.trigger)?,
            self.guards
                .into_iter()
                .map(GuardName::new)
                .collect::<Result<Vec<_>, _>>()?,
        )
    }
}
