use super::state_repeat_policy_document::StateRepeatPolicyDocument;
use made_core::error::DomainError;
use made_core::value_objects::{CeremonyState, StateExecution, StateId};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub(super) struct CeremonyStateDocument {
    id: String,
    #[serde(default)]
    initial: bool,
    #[serde(default)]
    terminal: bool,
    #[serde(default)]
    execution: StateExecution,
    #[serde(default)]
    repeat: Option<StateRepeatPolicyDocument>,
}

impl CeremonyStateDocument {
    pub(super) fn into_domain(self) -> Result<CeremonyState, DomainError> {
        if self.initial && self.terminal {
            return Err(DomainError::InvariantViolated {
                reason: "ceremony state cannot be both initial and terminal",
            });
        }
        let id = StateId::new(self.id)?;
        let state = if self.initial {
            CeremonyState::initial(id)
        } else if self.terminal {
            CeremonyState::terminal(id)
        } else {
            CeremonyState::intermediate(id)
        };
        let state = state.with_execution(self.execution);
        Ok(match self.repeat {
            Some(repeat) => state.with_repeat_policy(repeat.into_domain()?),
            None => state,
        })
    }
}
