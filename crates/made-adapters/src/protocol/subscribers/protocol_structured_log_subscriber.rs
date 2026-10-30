use async_trait::async_trait;
use made_core::ports::{PositionedRecord, ProtocolEventSubscriberPort};

/// Writes every sealed audit record as a structured info log entry.
#[derive(Debug, Default)]
pub struct ProtocolStructuredLogSubscriber;

impl ProtocolStructuredLogSubscriber {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ProtocolEventSubscriberPort for ProtocolStructuredLogSubscriber {
    async fn observe(&self, records: &[PositionedRecord]) {
        for positioned in records {
            match serde_json::to_string(&positioned.record) {
                Ok(record) => tracing::info!(
                    target: "made::protocol::audit",
                    global_position = positioned.position.value(),
                    protocol_id = %positioned.record.protocol_id(),
                    event_id = %positioned.record.event_id(),
                    trace_id = positioned.record.trace_id().unwrap_or_default(),
                    correlation_id = positioned.record.correlation_id().map(ToString::to_string).unwrap_or_default(),
                    causation_id = positioned.record.causation_id().map(ToString::to_string).unwrap_or_default(),
                    record = %record,
                    "protocol audit record"
                ),
                Err(error) => tracing::error!(
                    target: "made::protocol::audit",
                    error = %error,
                    "protocol audit record could not be serialized"
                ),
            }
        }
    }
}
