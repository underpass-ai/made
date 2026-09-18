use serde::{Deserialize, Serialize};

use crate::error::DomainError;

use super::{StepErrorMessage, StepFailureKind, StepOutput, StepStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepResult {
    status: StepStatus,
    output: StepOutput,
    error_message: Option<StepErrorMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    failure_kind: Option<StepFailureKind>,
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
            failure_kind: None,
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

    /// Preserve a known domain failure without classifying its prose.
    pub fn from_handler_error(error: &DomainError) -> Result<Self, DomainError> {
        let mut result = Self::failed(StepErrorMessage::new(error.to_string())?)?;
        if matches!(error, DomainError::NoValidProposal { .. }) {
            result.failure_kind = Some(StepFailureKind::NoValidProposal);
        }
        Ok(result)
    }

    #[must_use]
    pub fn failure_kind(&self) -> Option<StepFailureKind> {
        self.failure_kind
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::{ceremony_events::StepFailed, CeremonyEvent, CeremonyEventReader};
    use crate::value_objects::{
        AuditEventType, EventSchemaVersion, RoleId, StateIteration, StateVisit, StepAttempt,
        StepId, StepIteration,
    };

    #[test]
    fn only_typed_domain_failure_is_classified() {
        let typed = StepResult::from_handler_error(&DomainError::NoValidProposal {
            contract_id: "contract".to_owned(),
        })
        .unwrap();
        assert_eq!(typed.failure_kind(), Some(StepFailureKind::NoValidProposal));
        let prose =
            StepResult::failed(StepErrorMessage::new("NoValidProposal no_valid_proposal").unwrap())
                .unwrap();
        assert_eq!(prose.failure_kind(), None);
        let bytes = serde_json::to_string(&prose).unwrap();
        assert_eq!(
            bytes,
            r#"{"status":"FAILED","output":{},"error_message":"NoValidProposal no_valid_proposal"}"#
        );
        assert_eq!(serde_json::from_str::<StepResult>(&bytes).unwrap(), prose);
    }

    #[test]
    fn classified_failure_requires_its_new_event_version_and_legacy_stays_readable() {
        let mut failed = StepFailed {
            step_id: StepId::new("review").unwrap(),
            state_visit: Some(StateVisit::FIRST),
            state_iteration: Some(StateIteration::FIRST),
            iteration: StepIteration::FIRST,
            attempt: StepAttempt::FIRST,
            result: StepResult::from_handler_error(&DomainError::NoValidProposal {
                contract_id: "contract".to_owned(),
            })
            .unwrap(),
            finished_by: RoleId::new("reviewer").unwrap(),
            finished_at: time::OffsetDateTime::UNIX_EPOCH,
        };
        let event = CeremonyEvent::StepFailed(failed.clone());
        assert_eq!(event.schema_version(), EventSchemaVersion::V4);
        let raw = serde_json::to_value(&event).unwrap();
        assert!(CeremonyEventReader::read(
            AuditEventType::StepFailed,
            EventSchemaVersion::V3,
            raw.clone()
        )
        .is_err());
        assert_eq!(
            CeremonyEventReader::read(AuditEventType::StepFailed, EventSchemaVersion::V4, raw)
                .unwrap(),
            event
        );
        failed.result = StepResult::failed(StepErrorMessage::new("old failure").unwrap()).unwrap();
        let legacy = CeremonyEvent::StepFailed(failed);
        assert_eq!(legacy.schema_version(), EventSchemaVersion::V3);
        let raw = serde_json::to_value(&legacy).unwrap();
        assert_eq!(
            CeremonyEventReader::read(AuditEventType::StepFailed, EventSchemaVersion::V3, raw)
                .unwrap(),
            legacy
        );
    }
}
