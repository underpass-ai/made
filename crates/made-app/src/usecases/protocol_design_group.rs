use made_core::value_objects::{StateExecution, StepId};

use super::{ProtocolDesignGroupRepeat, ProtocolDesignGroupStep, ProtocolDesignJoin};

#[derive(Debug, Clone, PartialEq)]
pub struct ProtocolDesignGroup {
    id: StepId,
    execution: StateExecution,
    steps: Vec<ProtocolDesignGroupStep>,
    join: ProtocolDesignJoin,
    repeat: Option<ProtocolDesignGroupRepeat>,
}

impl ProtocolDesignGroup {
    #[must_use]
    pub fn new(
        id: StepId,
        execution: StateExecution,
        steps: Vec<ProtocolDesignGroupStep>,
        join: ProtocolDesignJoin,
    ) -> Self {
        Self {
            id,
            execution,
            steps,
            join,
            repeat: None,
        }
    }
    #[must_use]
    pub fn id(&self) -> &StepId {
        &self.id
    }
    #[must_use]
    pub fn execution(&self) -> StateExecution {
        self.execution
    }
    #[must_use]
    pub fn steps(&self) -> &[ProtocolDesignGroupStep] {
        &self.steps
    }
    #[must_use]
    pub fn join(&self) -> ProtocolDesignJoin {
        self.join
    }
    #[must_use]
    pub fn with_repeat(mut self, repeat: ProtocolDesignGroupRepeat) -> Self {
        self.repeat = Some(repeat);
        self
    }
    #[must_use]
    pub fn repeat(&self) -> Option<&ProtocolDesignGroupRepeat> {
        self.repeat.as_ref()
    }
}
