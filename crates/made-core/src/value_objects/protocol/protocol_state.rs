use serde::{Deserialize, Serialize};

use super::{ProtocolStateKind, StateExecution, StateId, StateRepeatPolicy};
use crate::value_objects::Attributes;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolState {
    id: StateId,
    kind: ProtocolStateKind,
    #[serde(default, skip_serializing_if = "StateExecution::is_sequential")]
    execution: StateExecution,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    repeat: Option<StateRepeatPolicy>,
    #[serde(default, skip_serializing_if = "Attributes::is_empty")]
    annotations: Attributes,
}

impl ProtocolState {
    #[must_use]
    pub fn new(id: StateId, kind: ProtocolStateKind) -> Self {
        Self {
            id,
            kind,
            execution: StateExecution::Sequential,
            repeat: None,
            annotations: Attributes::empty(),
        }
    }

    #[must_use]
    pub fn initial(id: StateId) -> Self {
        Self::new(id, ProtocolStateKind::Initial)
    }

    #[must_use]
    pub fn intermediate(id: StateId) -> Self {
        Self::new(id, ProtocolStateKind::Intermediate)
    }

    #[must_use]
    pub fn terminal(id: StateId) -> Self {
        Self::new(id, ProtocolStateKind::Terminal)
    }

    #[must_use]
    pub fn id(&self) -> &StateId {
        &self.id
    }

    #[must_use]
    pub fn kind(&self) -> ProtocolStateKind {
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

    /// Opaque authoring and rendering metadata. The state machine never
    /// interprets these values when deciding or folding commands.
    #[must_use]
    pub fn with_annotations(mut self, annotations: Attributes) -> Self {
        self.annotations = annotations;
        self
    }

    #[must_use]
    pub fn annotations(&self) -> &Attributes {
        &self.annotations
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
