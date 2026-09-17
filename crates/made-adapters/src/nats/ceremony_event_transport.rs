use async_nats::Client;
use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{CeremonyEventTransportPort, PositionedRecord};

use crate::ceremony_event_wire::CeremonyEventWire;

use super::NatsSubjects;

/// Publishes complete ceremony records on subjects grouped by event type.
#[derive(Debug, Clone)]
pub struct NatsCeremonyEventTransport {
    client: Client,
    subjects: NatsSubjects,
}

impl NatsCeremonyEventTransport {
    #[must_use]
    pub fn new(client: Client, subjects: NatsSubjects) -> Self {
        Self { client, subjects }
    }
}

#[async_trait]
impl CeremonyEventTransportPort for NatsCeremonyEventTransport {
    async fn deliver(&self, record: &PositionedRecord) -> Result<(), DomainError> {
        let payload = serde_json::to_vec(&CeremonyEventWire::from(record)).map_err(|_| {
            DomainError::InvariantViolated {
                reason: "nats: failed to serialize ceremony event",
            }
        })?;
        let subject = format!(
            "{}.{}",
            self.subjects.ceremony_prefix,
            record.record.event_type().as_str()
        );
        self.client
            .publish(subject, payload.into())
            .await
            .map_err(|_| DomainError::InvariantViolated {
                reason: "nats: ceremony event publish failed",
            })
    }
}
