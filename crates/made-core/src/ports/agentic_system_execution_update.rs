use crate::entities::AgenticSystemExecution;

/// What happened when a run was written back.
///
/// A conflict carries the run as it actually stands, so a caller that
/// lost the race can re-read what happened while it was deciding
/// instead of asking again and racing once more.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgenticSystemExecutionUpdate {
    Updated(AgenticSystemExecution),
    Conflict(AgenticSystemExecution),
}

impl AgenticSystemExecutionUpdate {
    #[must_use]
    pub const fn execution(&self) -> &AgenticSystemExecution {
        match self {
            Self::Updated(execution) | Self::Conflict(execution) => execution,
        }
    }

    #[must_use]
    pub fn into_execution(self) -> AgenticSystemExecution {
        match self {
            Self::Updated(execution) | Self::Conflict(execution) => execution,
        }
    }

    #[must_use]
    pub const fn is_conflict(&self) -> bool {
        matches!(self, Self::Conflict(_))
    }
}
