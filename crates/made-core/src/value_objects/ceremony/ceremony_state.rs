use serde::{Deserialize, Serialize};

use super::{CeremonyStateKind, StateExecution, StateId, StateRepeatPolicy};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyState {
    id: StateId,
    kind: CeremonyStateKind,
    #[serde(default, skip_serializing_if = "StateExecution::is_sequential")]
    execution: StateExecution,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    repeat: Option<StateRepeatPolicy>,
}

impl CeremonyState {
    #[must_use]
    pub fn new(id: StateId, kind: CeremonyStateKind) -> Self {
        Self {
            id,
            kind,
            execution: StateExecution::Sequential,
            repeat: None,
        }
    }

    #[must_use]
    pub fn initial(id: StateId) -> Self {
        Self::new(id, CeremonyStateKind::Initial)
    }

    #[must_use]
    pub fn intermediate(id: StateId) -> Self {
        Self::new(id, CeremonyStateKind::Intermediate)
    }

    #[must_use]
    pub fn terminal(id: StateId) -> Self {
        Self::new(id, CeremonyStateKind::Terminal)
    }

    #[must_use]
    pub fn id(&self) -> &StateId {
        &self.id
    }

    #[must_use]
    pub fn kind(&self) -> CeremonyStateKind {
        self.kind
    }

    #[must_use]
    pub fn with_execution(mut self, execution: StateExecution) -> Self {
        self.execution = execution;
        self
    }

    #[must_use]
    pub fn execution(&self) -> StateExecution {
        self.execution
    }

    #[must_use]
    pub fn with_repeat_policy(mut self, repeat: StateRepeatPolicy) -> Self {
        self.repeat = Some(repeat);
        self
    }

    #[must_use]
    pub fn repeat_policy(&self) -> Option<&StateRepeatPolicy> {
        self.repeat.as_ref()
    }

    #[must_use]
    pub fn is_initial(&self) -> bool {
        self.kind.is_initial()
    }

    #[must_use]
    pub fn is_terminal(&self) -> bool {
        self.kind.is_terminal()
    }
}
