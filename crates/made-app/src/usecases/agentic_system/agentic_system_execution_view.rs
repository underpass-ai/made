use made_core::entities::{AgenticSystem, AgenticSystemExecution};
use made_core::value_objects::ExecutionState;

use super::AgenticSystemProtocolView;

/// A run: the design it is running, what it recorded, and what the
/// instances themselves say.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgenticSystemExecutionView {
    execution: AgenticSystemExecution,
    system: AgenticSystem,
    protocols: Vec<AgenticSystemProtocolView>,
}

impl AgenticSystemExecutionView {
    #[must_use]
    pub const fn new(
        execution: AgenticSystemExecution,
        system: AgenticSystem,
        protocols: Vec<AgenticSystemProtocolView>,
    ) -> Self {
        Self {
            execution,
            system,
            protocols,
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
    pub fn protocols(&self) -> &[AgenticSystemProtocolView] {
        &self.protocols
    }

    #[must_use]
    pub fn state(&self) -> ExecutionState {
        self.execution.state()
    }
}
