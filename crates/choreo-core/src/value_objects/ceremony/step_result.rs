use serde::{Deserialize, Serialize};

use crate::error::DomainError;

use super::{StepErrorMessage, StepOutput, StepStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepResult {
    status: StepStatus,
    output: StepOutput,
    error_message: Option<StepErrorMessage>,
}

impl StepResult {
    pub fn new(
        status: StepStatus,
        output: StepOutput,
        error_message: Option<StepErrorMessage>,
    ) -> Result<Self, DomainError> {
        if matches!(status, StepStatus::Pending | StepStatus::InProgress) {
            return Err(DomainError::InvariantViolated {
                reason: "step result status must be observable",
            });
        }
        if status == StepStatus::Failed && error_message.is_none() {
            return Err(DomainError::EmptyField {
                field: "step_result.error_message",
            });
        }
        if status != StepStatus::Failed && error_message.is_some() {
            return Err(DomainError::InvariantViolated {
                reason: "only failed step results may carry an error message",
            });
        }
        Ok(Self {
            status,
            output,
            error_message,
        })
    }

    pub fn completed(output: StepOutput) -> Result<Self, DomainError> {
        Self::new(StepStatus::Completed, output, None)
    }

    pub fn waiting_for_human(output: StepOutput) -> Result<Self, DomainError> {
        Self::new(StepStatus::WaitingForHuman, output, None)
    }

    pub fn failed(error_message: StepErrorMessage) -> Result<Self, DomainError> {
        Self::new(StepStatus::Failed, StepOutput::empty(), Some(error_message))
    }

    #[must_use]
    pub fn status(&self) -> StepStatus {
        self.status
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
    pub fn into_parts(self) -> (StepStatus, StepOutput, Option<StepErrorMessage>) {
        (self.status, self.output, self.error_message)
    }

    #[must_use]
    pub fn is_success(&self) -> bool {
        self.status.is_success()
    }
}
