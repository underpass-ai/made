use made_core::value_objects::ExecutionRecoveryCursor;

use super::RecoverableCeremonyWorkerOutcome;

/// Results drained by one bounded scheduling pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyWorkerBatchOutcome {
    outcomes: Vec<RecoverableCeremonyWorkerOutcome>,
    next_cursor: Option<ExecutionRecoveryCursor>,
    stopped: bool,
}

impl CeremonyWorkerBatchOutcome {
    #[must_use]
    pub const fn new(
        outcomes: Vec<RecoverableCeremonyWorkerOutcome>,
        next_cursor: Option<ExecutionRecoveryCursor>,
        stopped: bool,
    ) -> Self {
        Self {
            outcomes,
            next_cursor,
            stopped,
        }
    }

    #[must_use]
    pub fn outcomes(&self) -> &[RecoverableCeremonyWorkerOutcome] {
        &self.outcomes
    }

    #[must_use]
    pub const fn next_cursor(&self) -> Option<&ExecutionRecoveryCursor> {
        self.next_cursor.as_ref()
    }

    #[must_use]
    pub const fn stopped(&self) -> bool {
        self.stopped
    }
}
