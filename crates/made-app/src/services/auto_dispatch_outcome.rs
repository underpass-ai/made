use made_core::entities::Deliberation;
use made_core::error::DomainError;
use made_core::value_objects::{Specialty, TaskId};

/// Per-specialty result of processing one trigger event.
#[derive(Debug, Clone, Default)]
pub struct AutoDispatchOutcome {
    pub successes: Vec<(Specialty, Deliberation)>,
    pub failures: Vec<(Specialty, DomainError)>,
}

impl AutoDispatchOutcome {
    #[must_use]
    pub fn accepted(&self) -> bool {
        !self.successes.is_empty()
    }

    #[must_use]
    pub fn dispatched_task_ids(&self) -> Vec<TaskId> {
        self.successes
            .iter()
            .map(|(_, deliberation)| deliberation.task_id().clone())
            .collect()
    }
}
