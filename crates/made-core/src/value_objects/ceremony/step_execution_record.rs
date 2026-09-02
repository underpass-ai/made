use serde::{Deserialize, Serialize};

use time::OffsetDateTime;

use super::{
    StepAttempt, StepErrorMessage, StepIteration, StepLease, StepOutput, StepResult, StepStatus,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepExecutionRecord {
    status: StepStatus,
    #[serde(default)]
    iteration: StepIteration,
    attempt: StepAttempt,
    lease: Option<StepLease>,
    output: StepOutput,
    error_message: Option<StepErrorMessage>,
}

impl StepExecutionRecord {
    #[must_use]
    pub fn pending() -> Self {
        Self {
            status: StepStatus::Pending,
            iteration: StepIteration::FIRST,
            attempt: StepAttempt::FIRST,
            lease: None,
            output: StepOutput::empty(),
            error_message: None,
        }
    }

    #[must_use]
    pub fn pending_iteration(iteration: StepIteration) -> Self {
        Self {
            iteration,
            ..Self::pending()
        }
    }

    #[must_use]
    pub fn status(&self) -> StepStatus {
        self.status
    }

    #[must_use]
    pub fn iteration(&self) -> StepIteration {
        self.iteration
    }

    #[must_use]
    pub fn attempt(&self) -> StepAttempt {
        self.attempt
    }

    #[must_use]
    pub fn lease(&self) -> Option<&StepLease> {
        self.lease.as_ref()
    }

    #[must_use]
    pub fn output(&self) -> &StepOutput {
        &self.output
    }

    #[must_use]
    pub fn error_message(&self) -> Option<&StepErrorMessage> {
        self.error_message.as_ref()
    }

    #[must_use]
    pub fn can_be_started_at(&self, now: OffsetDateTime) -> bool {
        if self.status.is_executable() {
            return true;
        }
        self.status == StepStatus::InProgress
            && self
                .lease
                .as_ref()
                .is_some_and(|lease| lease.is_expired_at(now))
    }

    #[must_use]
    pub fn with_started(self, lease: StepLease, attempt: StepAttempt) -> Self {
        Self {
            status: StepStatus::InProgress,
            iteration: self.iteration,
            attempt,
            lease: Some(lease),
            output: self.output,
            error_message: None,
        }
    }

    #[must_use]
    pub fn with_result(self, result: StepResult) -> Self {
        let (status, output, error_message) = result.into_parts();
        Self {
            status,
            iteration: self.iteration,
            attempt: self.attempt,
            lease: None,
            output,
            error_message,
        }
    }
}
