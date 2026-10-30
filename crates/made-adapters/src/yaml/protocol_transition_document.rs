use made_core::error::DomainError;
use made_core::value_objects::{GuardName, ProtocolTransition, StateId, TransitionTrigger};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub(super) struct ProtocolTransitionDocument {
    from: String,
    to: String,
    trigger: String,
    #[serde(default)]
    guards: Vec<String>,
}

impl ProtocolTransitionDocument {
    pub(super) fn into_domain(self) -> Result<ProtocolTransition, DomainError> {
        ProtocolTransition::new(
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
