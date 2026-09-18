use std::sync::Arc;

use made_core::entities::CeremonyEvent;
use made_core::error::DomainError;
use made_core::ports::{CeremonyEventCursorPort, CeremonyEventStorePort, ClockPort};
use made_core::value_objects::{
    CeremonyEventConsumer, CeremonyEventCursorLeaseId, CeremonyEventPageLimit, DurationMs,
};

use super::{
    AcceptChildCompletionInput, AcceptChildCompletionUseCase, PrepareCeremonyChildrenUseCase,
    RecoverCeremonyChildrenRound,
};
use crate::services::SessionStream;

const LEASE_DURATION: DurationMs = DurationMs::from_millis(30_000);

/// Drains the durable global feed for child plans and completed children.
///
/// A broker notification may call this method, but the notification carries no
/// authority: every decision is rebuilt from the event store and progress is
/// recorded only by the named cursor after the parent effect succeeds.
pub struct RecoverCeremonyChildrenUseCase {
    events: Arc<dyn CeremonyEventStorePort>,
    cursors: Arc<dyn CeremonyEventCursorPort>,
    stream: Arc<SessionStream>,
    prepare: Arc<PrepareCeremonyChildrenUseCase>,
    accept: Arc<AcceptChildCompletionUseCase>,
    clock: Arc<dyn ClockPort>,
    consumer: CeremonyEventConsumer,
}

impl RecoverCeremonyChildrenUseCase {
    #[must_use]
    pub fn new(
        events: Arc<dyn CeremonyEventStorePort>,
        cursors: Arc<dyn CeremonyEventCursorPort>,
        stream: Arc<SessionStream>,
        prepare: Arc<PrepareCeremonyChildrenUseCase>,
        accept: Arc<AcceptChildCompletionUseCase>,
        clock: Arc<dyn ClockPort>,
        consumer: CeremonyEventConsumer,
    ) -> Self {
        Self {
            events,
            cursors,
            stream,
            prepare,
            accept,
            clock,
            consumer,
        }
    }

    pub async fn execute(
        &self,
        limit: CeremonyEventPageLimit,
    ) -> Result<RecoverCeremonyChildrenRound, DomainError> {
        let mut round = RecoverCeremonyChildrenRound::default();
        let one = CeremonyEventPageLimit::new(1)?;
        for _ in 0..limit.value() {
            let lease_id = CeremonyEventCursorLeaseId::new(uuid::Uuid::new_v4().to_string())?;
            let Some(lease) = self
                .cursors
                .lease(&self.consumer, lease_id, self.clock.now(), LEASE_DURATION)
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

            let effect = self.process(&positioned.record).await;
            match effect {
                Ok(RecoveryEffect::PlanRecovered) => round.recovered_plans += 1,
                Ok(RecoveryEffect::CompletionAccepted) => round.accepted_completions += 1,
                Ok(RecoveryEffect::Skipped) => round.skipped += 1,
                Err(error) => {
                    self.cursors
                        .mark_failed(&lease, positioned.position)
                        .await?;
                    round.failed += 1;
                    tracing::warn!(
                        position = positioned.position.value(),
                        error = %error,
                        "child ceremony recovery left the cursor pending"
                    );
                    break;
                }
            }
            self.cursors
                .acknowledge_lease(&lease, positioned.position)
                .await?;
        }
        Ok(round)
    }

    async fn process(
        &self,
        record: &made_core::entities::AuditRecord,
    ) -> Result<RecoveryEffect, DomainError> {
        match record.event() {
            Some(CeremonyEvent::ChildSpawnPlanned(event)) => {
                self.prepare
                    .recover_group(record.ceremony_id(), event.plan.group_id())
                    .await?;
                Ok(RecoveryEffect::PlanRecovered)
            }
            Some(CeremonyEvent::ChildSpawnPlanAdopted(event)) => {
                self.prepare
                    .recover_group(record.ceremony_id(), &event.group_id)
                    .await?;
                Ok(RecoveryEffect::PlanRecovered)
            }
            Some(CeremonyEvent::CeremonyCompleted(_)) => {
                let child = self.stream.load(record.ceremony_id()).await?.instance;
                if child.lineage().is_none() {
                    return Ok(RecoveryEffect::Skipped);
                }
                self.accept
                    .execute(AcceptChildCompletionInput::new(
                        record.ceremony_id().clone(),
                        record.event_id().clone(),
                    ))
                    .await?;
                Ok(RecoveryEffect::CompletionAccepted)
            }
            _ => Ok(RecoveryEffect::Skipped),
        }
    }

    #[must_use]
    pub fn consumer(&self) -> &CeremonyEventConsumer {
        &self.consumer
    }
}

impl std::fmt::Debug for RecoverCeremonyChildrenUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RecoverCeremonyChildrenUseCase")
            .field("consumer", &self.consumer)
            .finish()
    }
}

enum RecoveryEffect {
    PlanRecovered,
    CompletionAccepted,
    Skipped,
}
