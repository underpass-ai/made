use async_trait::async_trait;
use made_core::ports::{CeremonyEventSubscriberPort, PositionedRecord};

/// Writes every sealed audit record as a structured info log entry.
#[derive(Debug, Default)]
pub struct CeremonyStructuredLogSubscriber;

impl CeremonyStructuredLogSubscriber {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CeremonyEventSubscriberPort for CeremonyStructuredLogSubscriber {
    async fn observe(&self, records: &[PositionedRecord]) {
        for positioned in records {
            match serde_json::to_string(&positioned.record) {
                Ok(record) => tracing::info!(
                    target: "made::ceremony::audit",
                    global_position = positioned.position.value(),
                    ceremony_id = %positioned.record.ceremony_id(),
                    event_id = %positioned.record.event_id(),
                    trace_id = positioned.record.trace_id().unwrap_or_default(),
                    correlation_id = positioned.record.correlation_id().map(ToString::to_string).unwrap_or_default(),
                    causation_id = positioned.record.causation_id().map(ToString::to_string).unwrap_or_default(),
                    record = %record,
                    "ceremony audit record"
                ),
                Err(error) => tracing::error!(
                    target: "made::ceremony::audit",
                    error = %error,
                    "ceremony audit record could not be serialized"
                ),
            }
        }
    }
}
