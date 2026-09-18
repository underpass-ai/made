use made_core::events::{
    DeliberationCompletedEvent, PhaseChangedEvent, TaskCompletedEvent, TaskDispatchedEvent,
    TaskFailedEvent,
};

/// One message accepted by the in-process publisher, with its complete payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InMemoryMessage {
    TaskDispatched(TaskDispatchedEvent),
    TaskCompleted(TaskCompletedEvent),
    TaskFailed(TaskFailedEvent),
    DeliberationCompleted(DeliberationCompletedEvent),
    PhaseChanged(PhaseChangedEvent),
}

impl InMemoryMessage {
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::TaskDispatched(_) => "task.dispatched",
            Self::TaskCompleted(_) => "task.completed",
            Self::TaskFailed(_) => "task.failed",
            Self::DeliberationCompleted(_) => "deliberation.completed",
            Self::PhaseChanged(_) => "phase.changed",
        }
    }
}
