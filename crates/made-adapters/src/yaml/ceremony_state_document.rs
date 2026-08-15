use made_core::error::DomainError;
use made_core::value_objects::{CeremonyState, StateId};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub(super) struct CeremonyStateDocument {
    id: String,
    #[serde(default)]
    initial: bool,
    #[serde(default)]
    terminal: bool,
}

impl CeremonyStateDocument {
    pub(super) fn into_domain(self) -> Result<CeremonyState, DomainError> {
        if self.initial && self.terminal {
            return Err(DomainError::InvariantViolated {
                reason: "ceremony state cannot be both initial and terminal",
            });
        }
        let id = StateId::new(self.id)?;
        Ok(if self.initial {
            CeremonyState::initial(id)
        } else if self.terminal {
            CeremonyState::terminal(id)
        } else {
            CeremonyState::intermediate(id)
        })
    }
}
