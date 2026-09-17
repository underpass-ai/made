use made_core::value_objects::{StateExecution, StepId};

use super::{CeremonyDesignGroupStep, CeremonyDesignJoin};

#[derive(Debug, Clone, PartialEq)]
pub struct CeremonyDesignGroup {
    id: StepId,
    execution: StateExecution,
    steps: Vec<CeremonyDesignGroupStep>,
    join: CeremonyDesignJoin,
}

impl CeremonyDesignGroup {
    #[must_use]
    pub fn new(
        id: StepId,
        execution: StateExecution,
        steps: Vec<CeremonyDesignGroupStep>,
        join: CeremonyDesignJoin,
    ) -> Self {
        Self {
            id,
            execution,
            steps,
            join,
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
    pub fn steps(&self) -> &[CeremonyDesignGroupStep] {
        &self.steps
    }
    #[must_use]
    pub fn join(&self) -> CeremonyDesignJoin {
        self.join
    }
}
