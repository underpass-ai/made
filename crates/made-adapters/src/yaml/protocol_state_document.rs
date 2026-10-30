use super::state_repeat_policy_document::StateRepeatPolicyDocument;
use made_core::error::DomainError;
use made_core::value_objects::{Attributes, ProtocolState, StateExecution, StateId};
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize)]
pub(super) struct ProtocolStateDocument {
    id: String,
    #[serde(default)]
    initial: bool,
    #[serde(default)]
    terminal: bool,
    #[serde(default)]
    execution: StateExecution,
    #[serde(default)]
    repeat: Option<StateRepeatPolicyDocument>,
    #[serde(default, rename = "x-pattern")]
    pattern: Option<String>,
}

impl ProtocolStateDocument {
    pub(super) fn into_domain(self) -> Result<ProtocolState, DomainError> {
        if self.initial && self.terminal {
            return Err(DomainError::InvariantViolated {
                reason: "protocol state cannot be both initial and terminal",
            });
        }
        let id = StateId::new(self.id)?;
        let state = if self.initial {
            ProtocolState::initial(id)
        } else if self.terminal {
            ProtocolState::terminal(id)
        } else {
            ProtocolState::intermediate(id)
        };
        let state = state.with_execution(self.execution);
        let state = match self.pattern {
            Some(pattern) => state.with_annotations(Attributes::new(BTreeMap::from([(
                "x-pattern".to_owned(),
                json!(pattern),
            )]))?),
            None => state,
        };
        Ok(match self.repeat {
            Some(repeat) => state.with_repeat_policy(repeat.into_domain()?),
            None => state,
        })
    }
}
