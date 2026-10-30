use made_core::value_objects::ExecutionRecoveryCursor;

use super::{ProtocolWorkerItemFailure, RecoverableProtocolWorkerOutcome};

/// Results drained by one bounded scheduling pass.
#[derive(Debug, Clone, PartialEq)]
pub struct ProtocolWorkerBatchOutcome {
    outcomes: Vec<RecoverableProtocolWorkerOutcome>,
    failures: Vec<ProtocolWorkerItemFailure>,
    next_cursor: Option<ExecutionRecoveryCursor>,
    stopped: bool,
}

impl ProtocolWorkerBatchOutcome {
    #[must_use]
    pub const fn new(
        outcomes: Vec<RecoverableProtocolWorkerOutcome>,
        failures: Vec<ProtocolWorkerItemFailure>,
        next_cursor: Option<ExecutionRecoveryCursor>,
        stopped: bool,
    ) -> Self {
        Self {
            outcomes,
            failures,
            next_cursor,
            stopped,
        }
    }

    #[must_use]
    pub fn outcomes(&self) -> &[RecoverableProtocolWorkerOutcome] {
        &self.outcomes
    }

    #[must_use]
    pub fn failures(&self) -> &[ProtocolWorkerItemFailure] {
        &self.failures
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
