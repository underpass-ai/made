use crate::entities::AgenticSystemExecution;

/// What happened when a run was opened.
///
/// Instantiating with an identifier that already names a run is the
/// normal retry, not an error: a host that did not hear the answer
/// asks again, and the store hands back the run that exists rather
/// than opening a second one beside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgenticSystemExecutionCreation {
    Created(AgenticSystemExecution),
    AlreadyExists(AgenticSystemExecution),
}

impl AgenticSystemExecutionCreation {
    #[must_use]
    pub const fn execution(&self) -> &AgenticSystemExecution {
        match self {
            Self::Created(execution) | Self::AlreadyExists(execution) => execution,
        }
    }

    #[must_use]
    pub fn into_execution(self) -> AgenticSystemExecution {
        match self {
            Self::Created(execution) | Self::AlreadyExists(execution) => execution,
        }
    }

    #[must_use]
    pub const fn is_new(&self) -> bool {
        matches!(self, Self::Created(_))
    }
}
