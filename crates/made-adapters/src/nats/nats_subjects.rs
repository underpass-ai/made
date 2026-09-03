use made_core::error::DomainError;

const MAX_SUBJECT_LEN: usize = 256;

/// Derived subjects for every inbound and outbound channel declared
/// by the AsyncAPI spec.
#[derive(Debug, Clone)]
pub struct NatsSubjects {
    pub trigger: String,
    pub task_dispatched: String,
    pub task_completed: String,
    pub task_failed: String,
    pub deliberation_completed: String,
    pub phase_changed: String,
}

impl NatsSubjects {
    pub fn new(
        publish_prefix: impl Into<String>,
        trigger_subject: impl Into<String>,
    ) -> Result<Self, DomainError> {
        let prefix_raw: String = publish_prefix.into();
        let trigger_raw: String = trigger_subject.into();
        let prefix = Self::validate_subject(&prefix_raw, "nats.publish_prefix")?;
        let trigger = Self::validate_subject(&trigger_raw, "nats.trigger_subject")?;
        Ok(Self {
            trigger,
            task_dispatched: format!("{prefix}.task.dispatched"),
            task_completed: format!("{prefix}.task.completed"),
            task_failed: format!("{prefix}.task.failed"),
            deliberation_completed: format!("{prefix}.deliberation.completed"),
            phase_changed: format!("{prefix}.phase.changed"),
        })
    }

    fn validate_subject(raw: &str, field: &'static str) -> Result<String, DomainError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(DomainError::EmptyField { field });
        }
        if trimmed.len() > MAX_SUBJECT_LEN {
            return Err(DomainError::FieldTooLong {
                field,
                actual: trimmed.len(),
                max: MAX_SUBJECT_LEN,
            });
        }
        if trimmed.chars().any(char::is_control) {
            return Err(DomainError::InvalidCharacters { field });
        }
        Ok(trimmed.to_owned())
    }
}
