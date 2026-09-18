use made_core::entities::CouncilJournalEvent;
use made_core::error::DomainError;
use made_core::ports::{ClockPort, CouncilJournalPort, MessagingPort};
use made_core::value_objects::{
    CouncilJournalConsumer, CouncilJournalPageLimit, CouncilJournalPosition, DurationMs,
};
use std::sync::Arc;

/// At-least-once forwarding of the five messaging facts. Configuration and
/// snapshot facts advance this cursor without being invented as bus events.
pub struct PublishCouncilEventsUseCase {
    journal: Arc<dyn CouncilJournalPort>,
    transport: Arc<dyn MessagingPort>,
    clock: Arc<dyn ClockPort>,
}
impl std::fmt::Debug for PublishCouncilEventsUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PublishCouncilEventsUseCase")
            .finish_non_exhaustive()
    }
}
impl PublishCouncilEventsUseCase {
    #[must_use]
    pub fn new(
        journal: Arc<dyn CouncilJournalPort>,
        transport: Arc<dyn MessagingPort>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            journal,
            transport,
            clock,
        }
    }
    pub async fn execute(
        &self,
        consumer: &CouncilJournalConsumer,
        limit: CouncilJournalPageLimit,
    ) -> Result<Vec<CouncilJournalPosition>, DomainError> {
        let mut acknowledged = Vec::new();
        for _ in 0..limit.value() {
            let Some(lease) = self
                .journal
                .lease(consumer, self.clock.now(), DurationMs::from_millis(30_000))
                .await?
            else {
                break;
            };
            let records = self
                .journal
                .read(
                    lease.acknowledged_through(),
                    CouncilJournalPageLimit::new(1)?,
                )
                .await;
            let record = match records {
                Ok(records) => records.into_iter().next(),
                Err(error) => {
                    let _ = self.journal.release(&lease, self.clock.now()).await;
                    return Err(error);
                }
            };
            let Some(record) = record else {
                self.journal.release(&lease, self.clock.now()).await?;
                break;
            };
            let delivery = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                self.deliver(record.event()),
            )
            .await
            .map_err(|_| DomainError::InvariantViolated {
                reason: "council journal forwarding timed out; delivery remains unconfirmed",
            })
            .and_then(std::convert::identity);
            if let Err(error) = delivery {
                let _ = self.journal.release(&lease, self.clock.now()).await;
                return Err(error);
            }
            self.journal
                .acknowledge(&lease, record.position(), self.clock.now())
                .await?;
            acknowledged.push(record.position());
        }
        Ok(acknowledged)
    }
    async fn deliver(&self, event: &CouncilJournalEvent) -> Result<(), DomainError> {
        match event {
            CouncilJournalEvent::TaskDispatched(event) => {
                self.transport.publish_task_dispatched(event).await
            }
            CouncilJournalEvent::TaskCompleted(event) => {
                self.transport.publish_task_completed(event).await
            }
            CouncilJournalEvent::TaskFailed(event) => {
                self.transport.publish_task_failed(event).await
            }
            CouncilJournalEvent::DeliberationCompleted(event) => {
                self.transport.publish_deliberation_completed(event).await
            }
            CouncilJournalEvent::PhaseChanged(event) => {
                self.transport.publish_phase_changed(event).await
            }
            CouncilJournalEvent::SnapshotImported(_)
            | CouncilJournalEvent::CouncilRegistered(_)
            | CouncilJournalEvent::CouncilReplaced(_)
            | CouncilJournalEvent::CouncilDeleted(_)
            | CouncilJournalEvent::AgentRegistered(_)
            | CouncilJournalEvent::AgentUnregistered(_)
            | CouncilJournalEvent::ContractRegistered(_)
            | CouncilJournalEvent::ContractDeleted(_)
            | CouncilJournalEvent::DeliberationSnapshotSaved(_)
            | CouncilJournalEvent::StatisticsRecorded(_) => Ok(()),
        }
    }
}
