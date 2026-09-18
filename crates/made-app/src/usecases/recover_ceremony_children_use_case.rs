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

mod recovery_effect;
use recovery_effect::RecoveryEffect;

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
            let page = match self.events.read_all(lease.next_position(), one).await {
                Ok(page) => page,
                Err(error) => {
                    self.release_after_error(&lease, "read global ceremony events")
                        .await;
                    return Err(error);
                }
            };
            let Some(positioned) = page.into_iter().next() else {
                self.cursors.release(&lease).await?;
                break;
            };

            let effect = self.process(&positioned.record).await;
            match effect {
                Ok(RecoveryEffect::PlanRecovered) => round.recovered_plans += 1,
                Ok(RecoveryEffect::CompletionAccepted) => round.accepted_completions += 1,
                Ok(RecoveryEffect::Skipped) => round.skipped += 1,
                Err(error) => {
                    if let Err(cursor_error) =
                        self.cursors.mark_failed(&lease, positioned.position).await
                    {
                        self.release_after_error(&lease, "mark ceremony recovery failed")
                            .await;
                        return Err(cursor_error);
                    }
                    round.failed += 1;
                    tracing::warn!(
                        position = positioned.position.value(),
                        error = %error,
                        "child ceremony recovery left the cursor pending"
                    );
                    break;
                }
            }
            if let Err(error) = self
                .cursors
                .acknowledge_lease(&lease, positioned.position)
                .await
            {
                self.release_after_error(&lease, "acknowledge ceremony recovery")
                    .await;
                return Err(error);
            }
        }
        Ok(round)
    }

    async fn release_after_error(
        &self,
        lease: &made_core::value_objects::CeremonyEventCursorLease,
        operation: &'static str,
    ) {
        if let Err(error) = self.cursors.release(lease).await {
            tracing::warn!(%error, operation, "child ceremony recovery could not release its cursor lease");
        }
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

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;
    use made_core::entities::AuditFact;
    use made_core::ports::{
        AppendOutcome, CeremonyEventCursorPort, CeremonyEventSubscriberPort,
        NoopCeremonyEventSubscriber,
    };
    use made_core::value_objects::{
        AuditActorKind, CeremonyEventCursorAttempt, CeremonyEventQuarantineReason, GlobalPosition,
        IdempotencyKey, LeaseOwnerId, QuarantinedCeremonyEvent, StreamVersion,
    };
    use tokio::sync::Mutex;

    use super::*;
    use crate::usecases::ceremony_test_support::{
        a_memory, ceremony_id, child_spawning_definition, lease_ttl, now, resolver_with,
        review_child_definition, role_id, started_instance, step_id, EventStoreFake, FixedClock,
        PublicationsFake, StepHandlerFake,
    };
    use crate::usecases::{RunCeremonyStepInput, RunCeremonyStepUseCase};

    struct FailFirstChildOpening {
        store: Arc<EventStoreFake>,
        parent_id: made_core::value_objects::CeremonyId,
        failures_remaining: AtomicUsize,
    }

    struct FailFirstGlobalRead {
        store: Arc<EventStoreFake>,
        failures_remaining: AtomicUsize,
    }

    #[async_trait]
    impl CeremonyEventStorePort for FailFirstChildOpening {
        async fn append(
            &self,
            stream: &made_core::value_objects::CeremonyId,
            expected: StreamVersion,
            facts: Vec<AuditFact>,
        ) -> Result<AppendOutcome, DomainError> {
            if stream != &self.parent_id
                && self
                    .failures_remaining
                    .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
                        remaining.checked_sub(1)
                    })
                    .is_ok()
            {
                return Err(DomainError::InvariantViolated {
                    reason: "injected crash before child opening",
                });
            }
            self.store.append(stream, expected, facts).await
        }

        async fn read(
            &self,
            stream: &made_core::value_objects::CeremonyId,
            after: StreamVersion,
            limit: CeremonyEventPageLimit,
        ) -> Result<Vec<made_core::entities::AuditRecord>, DomainError> {
            self.store.read(stream, after, limit).await
        }

        async fn read_all(
            &self,
            from: GlobalPosition,
            limit: CeremonyEventPageLimit,
        ) -> Result<Vec<made_core::ports::PositionedRecord>, DomainError> {
            self.store.read_all(from, limit).await
        }

        async fn head(
            &self,
            stream: &made_core::value_objects::CeremonyId,
        ) -> Result<StreamVersion, DomainError> {
            self.store.head(stream).await
        }

        async fn streams(&self) -> Result<Vec<made_core::value_objects::CeremonyId>, DomainError> {
            self.store.streams().await
        }
    }

    #[async_trait]
    impl CeremonyEventStorePort for FailFirstGlobalRead {
        async fn append(
            &self,
            stream: &made_core::value_objects::CeremonyId,
            expected: StreamVersion,
            facts: Vec<AuditFact>,
        ) -> Result<AppendOutcome, DomainError> {
            self.store.append(stream, expected, facts).await
        }

        async fn read(
            &self,
            stream: &made_core::value_objects::CeremonyId,
            after: StreamVersion,
            limit: CeremonyEventPageLimit,
        ) -> Result<Vec<made_core::entities::AuditRecord>, DomainError> {
            self.store.read(stream, after, limit).await
        }

        async fn read_all(
            &self,
            from: GlobalPosition,
            limit: CeremonyEventPageLimit,
        ) -> Result<Vec<made_core::ports::PositionedRecord>, DomainError> {
            if self
                .failures_remaining
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
                    remaining.checked_sub(1)
                })
                .is_ok()
            {
                return Err(DomainError::InvariantViolated {
                    reason: "injected transient global read failure",
                });
            }
            self.store.read_all(from, limit).await
        }

        async fn head(
            &self,
            stream: &made_core::value_objects::CeremonyId,
        ) -> Result<StreamVersion, DomainError> {
            self.store.head(stream).await
        }

        async fn streams(&self) -> Result<Vec<made_core::value_objects::CeremonyId>, DomainError> {
            self.store.streams().await
        }
    }

    #[derive(Debug, Default)]
    struct TestCursor {
        state: Mutex<TestCursorState>,
    }

    #[derive(Debug, Default)]
    struct TestCursorState {
        position: Option<GlobalPosition>,
        lease: Option<made_core::value_objects::CeremonyEventCursorLease>,
        attempt: CeremonyEventCursorAttempt,
    }

    #[async_trait]
    impl CeremonyEventCursorPort for TestCursor {
        async fn position(
            &self,
            _consumer: &CeremonyEventConsumer,
        ) -> Result<Option<GlobalPosition>, DomainError> {
            Ok(self.state.lock().await.position)
        }

        async fn lease(
            &self,
            consumer: &CeremonyEventConsumer,
            lease_id: CeremonyEventCursorLeaseId,
            now: time::OffsetDateTime,
            duration: DurationMs,
        ) -> Result<Option<made_core::value_objects::CeremonyEventCursorLease>, DomainError>
        {
            let mut state = self.state.lock().await;
            if state.lease.is_some() {
                return Ok(None);
            }
            let lease = made_core::value_objects::CeremonyEventCursorLease::new(
                consumer.clone(),
                lease_id,
                state.position,
                state.attempt,
                now + time::Duration::milliseconds(i64::try_from(duration.get()).unwrap()),
            );
            state.lease = Some(lease.clone());
            Ok(Some(lease))
        }

        async fn acknowledge(
            &self,
            _consumer: &CeremonyEventConsumer,
            through: GlobalPosition,
        ) -> Result<(), DomainError> {
            let mut state = self.state.lock().await;
            state.position = Some(through);
            state.attempt = CeremonyEventCursorAttempt::NONE;
            Ok(())
        }

        async fn acknowledge_lease(
            &self,
            lease: &made_core::value_objects::CeremonyEventCursorLease,
            through: GlobalPosition,
        ) -> Result<(), DomainError> {
            let mut state = self.state.lock().await;
            if state
                .lease
                .as_ref()
                .is_none_or(|held| held.lease_id() != lease.lease_id())
            {
                return Err(DomainError::Conflict {
                    what: "test_cursor",
                });
            }
            state.position = Some(through);
            state.attempt = CeremonyEventCursorAttempt::NONE;
            state.lease = None;
            Ok(())
        }

        async fn mark_failed(
            &self,
            _lease: &made_core::value_objects::CeremonyEventCursorLease,
            _position: GlobalPosition,
        ) -> Result<(), DomainError> {
            let mut state = self.state.lock().await;
            state.attempt = state.attempt.next();
            state.lease = None;
            Ok(())
        }

        async fn quarantine(
            &self,
            _lease: &made_core::value_objects::CeremonyEventCursorLease,
            _position: GlobalPosition,
            _reason: CeremonyEventQuarantineReason,
            _now: time::OffsetDateTime,
        ) -> Result<(), DomainError> {
            Err(DomainError::InvariantViolated {
                reason: "test cursor does not quarantine",
            })
        }

        async fn release(
            &self,
            _lease: &made_core::value_objects::CeremonyEventCursorLease,
        ) -> Result<(), DomainError> {
            self.state.lock().await.lease = None;
            Ok(())
        }

        async fn quarantined(
            &self,
            _consumer: &CeremonyEventConsumer,
        ) -> Result<Vec<QuarantinedCeremonyEvent>, DomainError> {
            Ok(Vec::new())
        }
    }

    #[tokio::test]
    async fn a_transient_feed_read_releases_the_lease_for_the_next_tick() {
        let definition = child_spawning_definition();
        let definitions = Arc::new(
            crate::usecases::ceremony_test_support::DefinitionRepositoryFake::new(
                definition.clone(),
            ),
        );
        let publications = Arc::new(PublicationsFake::default());
        publications.seed(review_child_definition()).await;
        let store = Arc::new(EventStoreFake::default());
        store.save(&started_instance(&definition)).await.unwrap();
        let stream = Arc::new(SessionStream::new(
            store.clone(),
            store.clone(),
            Arc::new(NoopCeremonyEventSubscriber),
        ));
        let resolver = resolver_with(definitions, publications.clone());
        let prepare = Arc::new(PrepareCeremonyChildrenUseCase::new(
            resolver.clone(),
            publications.clone(),
            stream.clone(),
            Arc::new(FixedClock::new(now())),
            a_memory(),
        ));
        let accept = Arc::new(AcceptChildCompletionUseCase::new(
            resolver,
            publications,
            stream.clone(),
            Arc::new(FixedClock::new(now())),
        ));
        let cursor = Arc::new(TestCursor::default());
        let recover = RecoverCeremonyChildrenUseCase::new(
            Arc::new(FailFirstGlobalRead {
                store,
                failures_remaining: AtomicUsize::new(1),
            }),
            cursor.clone(),
            stream,
            prepare,
            accept,
            Arc::new(FixedClock::new(now())),
            CeremonyEventConsumer::new("children-transient-read").unwrap(),
        );

        let first = recover
            .execute(CeremonyEventPageLimit::new(1).unwrap())
            .await
            .unwrap_err();
        assert!(first.to_string().contains("transient global read"));

        let second = recover
            .execute(CeremonyEventPageLimit::new(1).unwrap())
            .await
            .unwrap();
        assert!(!second.busy);
        assert_eq!(second.skipped, 1);
        assert!(cursor.position(recover.consumer()).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn an_adoption_wake_recovers_after_a_crash_before_the_first_child_open() {
        let definition = child_spawning_definition();
        let definitions = Arc::new(
            crate::usecases::ceremony_test_support::DefinitionRepositoryFake::new(
                definition.clone(),
            ),
        );
        let publications = Arc::new(PublicationsFake::default());
        publications.seed(review_child_definition()).await;
        let store = Arc::new(EventStoreFake::default());
        store.save(&started_instance(&definition)).await.unwrap();
        let crashing_events = Arc::new(FailFirstChildOpening {
            store: store.clone(),
            parent_id: ceremony_id(),
            failures_remaining: AtomicUsize::new(1),
        });
        let crashing_stream = Arc::new(SessionStream::new(
            crashing_events,
            store.clone(),
            Arc::new(NoopCeremonyEventSubscriber) as Arc<dyn CeremonyEventSubscriberPort>,
        ));
        let resolver = resolver_with(definitions.clone(), publications.clone());
        let crashing_prepare = Arc::new(PrepareCeremonyChildrenUseCase::new(
            resolver.clone(),
            publications.clone(),
            crashing_stream.clone(),
            Arc::new(FixedClock::new(now())),
            a_memory(),
        ));
        let runner = RunCeremonyStepUseCase::new(
            resolver.clone(),
            crashing_stream,
            Arc::new(StepHandlerFake::failing(DomainError::InvariantViolated {
                reason: "spawn handler must not run",
            })),
            Arc::new(FixedClock::new(now())),
        )
        .with_child_orchestrator(crashing_prepare);

        let first_error = runner
            .execute(RunCeremonyStepInput::new(
                ceremony_id(),
                role_id(),
                AuditActorKind::Agent,
                step_id(),
                LeaseOwnerId::new("crashed-worker").unwrap(),
                IdempotencyKey::new("crash-after-plan").unwrap(),
                lease_ttl(),
            ))
            .await
            .unwrap_err();
        assert!(first_error.to_string().contains("before child opening"));

        let later = now() + time::Duration::seconds(120);
        let reclaimed_stream = Arc::new(SessionStream::new(
            Arc::new(FailFirstChildOpening {
                store: store.clone(),
                parent_id: ceremony_id(),
                failures_remaining: AtomicUsize::new(1),
            }),
            store.clone(),
            Arc::new(NoopCeremonyEventSubscriber),
        ));
        let reclaimed_prepare = Arc::new(PrepareCeremonyChildrenUseCase::new(
            resolver.clone(),
            publications.clone(),
            reclaimed_stream.clone(),
            Arc::new(FixedClock::new(later)),
            a_memory(),
        ));
        let reclaimed_runner = RunCeremonyStepUseCase::new(
            resolver.clone(),
            reclaimed_stream,
            Arc::new(StepHandlerFake::failing(DomainError::InvariantViolated {
                reason: "spawn handler must not run",
            })),
            Arc::new(FixedClock::new(later)),
        )
        .with_child_orchestrator(reclaimed_prepare);
        let adoption_error = reclaimed_runner
            .execute(RunCeremonyStepInput::new(
                ceremony_id(),
                role_id(),
                AuditActorKind::Agent,
                step_id(),
                LeaseOwnerId::new("replacement-worker").unwrap(),
                IdempotencyKey::new("crash-after-adoption").unwrap(),
                lease_ttl(),
            ))
            .await
            .unwrap_err();
        assert!(
            adoption_error.to_string().contains("before child opening"),
            "unexpected reclaim failure: {adoption_error}"
        );
        let before = store.saved(&ceremony_id()).await;
        let group = before.child_groups().values().next().unwrap();
        let child_id = group.plan().children()[0].child_id().clone();
        assert!(!store.exists(&child_id).await);

        let plan_position = store
            .read_all(GlobalPosition::FIRST, CeremonyEventPageLimit::DEFAULT)
            .await
            .unwrap()
            .into_iter()
            .find(|positioned| {
                matches!(
                    positioned.record.event(),
                    Some(CeremonyEvent::ChildSpawnPlanned(_))
                )
            })
            .unwrap()
            .position;
        let consumer = CeremonyEventConsumer::new("children-test").unwrap();
        let cursor = Arc::new(TestCursor::default());
        cursor.acknowledge(&consumer, plan_position).await.unwrap();
        let restarted_stream = Arc::new(SessionStream::new(
            store.clone(),
            store.clone(),
            Arc::new(NoopCeremonyEventSubscriber),
        ));
        let prepare = Arc::new(PrepareCeremonyChildrenUseCase::new(
            resolver.clone(),
            publications.clone(),
            restarted_stream.clone(),
            Arc::new(FixedClock::new(later)),
            a_memory(),
        ));
        let accept = Arc::new(AcceptChildCompletionUseCase::new(
            resolver,
            publications,
            restarted_stream,
            Arc::new(FixedClock::new(later)),
        ));
        let recover = RecoverCeremonyChildrenUseCase::new(
            store.clone(),
            cursor,
            Arc::new(SessionStream::new(
                store.clone(),
                store.clone(),
                Arc::new(NoopCeremonyEventSubscriber),
            )),
            prepare,
            accept,
            Arc::new(FixedClock::new(later)),
            consumer,
        );

        let round = recover
            .execute(CeremonyEventPageLimit::DEFAULT)
            .await
            .unwrap();
        assert!(round.recovered_plans >= 1, "the adoption wake was ignored");
        assert!(store.exists(&child_id).await);
        let after = store.saved(&ceremony_id()).await;
        assert_eq!(
            after.step_record(&step_id()).unwrap().status(),
            made_core::value_objects::StepStatus::Completed
        );
        let records = store.records(&ceremony_id()).await;
        assert_eq!(
            records
                .iter()
                .filter(|record| matches!(
                    record.event(),
                    Some(CeremonyEvent::ChildSpawnPlanned(_))
                ))
                .count(),
            1
        );
        assert_eq!(
            records
                .iter()
                .filter(|record| matches!(
                    record.event(),
                    Some(CeremonyEvent::ChildSpawnPlanAdopted(_))
                ))
                .count(),
            1
        );
    }
}
