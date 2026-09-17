use std::fmt;
use std::sync::Arc;
use std::time::Duration;

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
const RETRY_BACKOFF_MILLIS: u64 = 100;
const MAX_RETRY_BACKOFF_MILLIS: u64 = 400;

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
                .acknowledge_lease(&lease, positioned.position)
                .await?;
            round.delivered += 1;
        }
        Ok(round)
    }

    /// Publish a bounded number of positions without requiring another append
    /// to wake a failed cursor.
    ///
    /// Failed attempts do not consume `limit`: the same pending position is
    /// retried after bounded backoff until it is acknowledged or the durable
    /// cursor's existing policy quarantines it. No task is spawned; the caller
    /// owns and awaits the whole bounded operation.
    pub async fn execute_automatically(
        &self,
        consumer: &CeremonyEventConsumer,
        limit: CeremonyEventPageLimit,
    ) -> Result<PublishCeremonyEventsRound, DomainError> {
        let mut summary = PublishCeremonyEventsRound::default();
        let mut failures_without_progress = 0_u32;

        while summary.confirmed() < limit.value() {
            let remaining = CeremonyEventPageLimit::new(limit.value() - summary.confirmed())?;
            let round = self.execute(consumer, remaining).await?;
            summary.delivered += round.delivered;
            summary.quarantined += round.quarantined;
            summary.busy |= round.busy;

            if round.confirmed() > 0 {
                failures_without_progress = 0;
            }
            if round.failed > 0 {
                summary.retried += round.failed;
                failures_without_progress = failures_without_progress.saturating_add(1);
                if failures_without_progress > MAX_ATTEMPTS {
                    return Err(DomainError::InvariantViolated {
                        reason: "automatic ceremony event publication made no retry progress",
                    });
                }
                tracing::warn!(
                    consumer = consumer.as_str(),
                    attempt = failures_without_progress,
                    "ceremony event delivery failed; retrying the pending position"
                );
                tokio::time::sleep(retry_backoff(failures_without_progress)).await;
                continue;
            }
            if round.busy || round.confirmed() < remaining.value() {
                break;
            }
        }

        Ok(summary)
    }
}

fn retry_backoff(attempt: u32) -> Duration {
    let shift = attempt.saturating_sub(1).min(2);
    let millis = RETRY_BACKOFF_MILLIS
        .saturating_mul(1_u64 << shift)
        .min(MAX_RETRY_BACKOFF_MILLIS);
    Duration::from_millis(millis)
}

impl fmt::Debug for PublishCeremonyEventsUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PublishCeremonyEventsUseCase")
            .finish()
    }
}
