use made_core::entities::{AgenticSystem, AgenticSystemExecution};
use made_core::value_objects::ExecutionState;

use super::AgenticSystemCeremonyView;

/// A run: the design it is running, what it recorded, and what the
/// instances themselves say.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgenticSystemExecutionView {
    execution: AgenticSystemExecution,
    system: AgenticSystem,
    ceremonies: Vec<AgenticSystemCeremonyView>,
}

impl AgenticSystemExecutionView {
    #[must_use]
    pub const fn new(
        execution: AgenticSystemExecution,
        system: AgenticSystem,
        ceremonies: Vec<AgenticSystemCeremonyView>,
    ) -> Self {
        Self {
            execution,
            system,
            ceremonies,
        }
    }

    #[must_use]
    pub const fn execution(&self) -> &AgenticSystemExecution {
        &self.execution
    }

    /// The design as it was sealed, not as it is now.
    #[must_use]
    pub const fn system(&self) -> &AgenticSystem {
        &self.system
    }

    #[must_use]
    pub fn ceremonies(&self) -> &[AgenticSystemCeremonyView] {
        &self.ceremonies
    }

    #[must_use]
    pub fn state(&self) -> ExecutionState {
        self.execution.state()
    }
}
