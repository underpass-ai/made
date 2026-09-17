use std::fmt;
use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::{
    CeremonyEventCursorPort, CeremonyEventStorePort, CeremonyEventTransportPort, ClockPort,
};
use made_core::value_objects::{
    CeremonyEventConsumer, CeremonyEventCursorLeaseId, CeremonyEventPageLimit,
    CeremonyEventQuarantineReason, DurationMs,
};

use super::PublishCeremonyEventsRound;

const LEASE_DURATION: DurationMs = DurationMs::from_millis(30_000);
const MAX_ATTEMPTS: u32 = 3;

/// Publishes a named cursor in global order with at-least-once delivery.
pub struct PublishCeremonyEventsUseCase {
    events: Arc<dyn CeremonyEventStorePort>,
    cursors: Arc<dyn CeremonyEventCursorPort>,
    transport: Arc<dyn CeremonyEventTransportPort>,
    clock: Arc<dyn ClockPort>,
}

impl PublishCeremonyEventsUseCase {
    #[must_use]
    pub fn new(
        events: Arc<dyn CeremonyEventStorePort>,
        cursors: Arc<dyn CeremonyEventCursorPort>,
        transport: Arc<dyn CeremonyEventTransportPort>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            events,
            cursors,
            transport,
            clock,
        }
    }

    pub async fn execute(
        &self,
        consumer: &CeremonyEventConsumer,
        limit: CeremonyEventPageLimit,
    ) -> Result<PublishCeremonyEventsRound, DomainError> {
        let mut round = PublishCeremonyEventsRound::default();
        let one = CeremonyEventPageLimit::new(1)?;
        for _ in 0..limit.value() {
            let lease_id = CeremonyEventCursorLeaseId::new(uuid::Uuid::new_v4().to_string())?;
            let Some(lease) = self
                .cursors
                .lease(consumer, lease_id, self.clock.now(), LEASE_DURATION)
                .await?
            else {
                round.busy = true;
                break;
            };
            let Some(positioned) = self
                .events
                .read_all(lease.next_position(), one)
                .await?
                .into_iter()
                .next()
            else {
                self.cursors.release(&lease).await?;
                break;
            };
            if lease.attempt().is_exhausted(MAX_ATTEMPTS) {
                self.cursors
                    .quarantine(
                        &lease,
                        positioned.position,
                        CeremonyEventQuarantineReason::new("delivery failed three times")?,
                        self.clock.now(),
                    )
                    .await?;
                round.quarantined += 1;
                continue;
            }
            if self.transport.deliver(&positioned).await.is_err() {
                self.cursors
                    .mark_failed(&lease, positioned.position)
                    .await?;
                round.failed += 1;
                break;
            }
            self.cursors
                .acknowledge(consumer, positioned.position)
                .await?;
            round.delivered += 1;
        }
        Ok(round)
    }
}

impl fmt::Debug for PublishCeremonyEventsUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PublishCeremonyEventsUseCase")
            .finish()
    }
}
