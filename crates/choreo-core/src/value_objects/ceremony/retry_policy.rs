use serde::{Deserialize, Serialize};

use crate::value_objects::DurationMs;

use super::StepAttempt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryPolicy {
    max_attempts: StepAttempt,
    backoff: DurationMs,
}

impl RetryPolicy {
    #[must_use]
    pub fn new(max_attempts: StepAttempt, backoff: DurationMs) -> Self {
        Self {
            max_attempts,
            backoff,
        }
    }

    #[must_use]
    pub fn single_attempt() -> Self {
        Self::new(StepAttempt::FIRST, DurationMs::ZERO)
    }

    #[must_use]
    pub fn max_attempts(self) -> StepAttempt {
        self.max_attempts
    }

    #[must_use]
    pub fn backoff(self) -> DurationMs {
        self.backoff
    }

    #[must_use]
    pub fn allows_attempt(self, attempt: StepAttempt) -> bool {
        attempt <= self.max_attempts
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::single_attempt()
    }
}
