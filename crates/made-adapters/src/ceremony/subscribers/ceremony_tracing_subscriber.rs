use async_trait::async_trait;
use made_core::ports::{CeremonyEventSubscriberPort, PositionedRecord};

/// Emits one tracing event for every record while the append span is current.
#[derive(Debug, Default)]
pub struct CeremonyTracingSubscriber;

impl CeremonyTracingSubscriber {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CeremonyEventSubscriberPort for CeremonyTracingSubscriber {
    async fn observe(&self, records: &[PositionedRecord]) {
        for positioned in records {
            let record = &positioned.record;
            let payload = record
                .event()
                .and_then(|event| serde_json::to_string(event).ok())
                .unwrap_or_default();
            tracing::event!(
                target: "made::ceremony::event",
                tracing::Level::INFO,
                global_position = positioned.position.value(),
                ceremony_id = %record.ceremony_id(),
                event_id = %record.event_id(),
                event_type = record.event_type().as_str(),
                sequence = record.sequence().value(),
                trace_id = record.trace_id().unwrap_or_default(),
                correlation_id = record.correlation_id().map(ToString::to_string).unwrap_or_default(),
                causation_id = record.causation_id().map(ToString::to_string).unwrap_or_default(),
                payload = %payload,
                "ceremony event sealed"
            );
        }
    }
}
