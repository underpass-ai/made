use std::sync::Arc;

use made_core::entities::ProtocolEvent;
use made_core::error::DomainError;
use made_core::ports::{ClockPort, ProtocolEventCursorPort, ProtocolEventStorePort};
use made_core::value_objects::{
    DurationMs, ProtocolEventConsumer, ProtocolEventCursorLeaseId, ProtocolEventPageLimit,
};

use super::{
    AcceptChildCompletionInput, AcceptChildCompletionUseCase, PrepareProtocolChildrenUseCase,
    RecoverProtocolChildrenRound,
};
use crate::authorization::ContinueAcceptedProtocolWorkUseCase;
use crate::services::AuthorizationOperationScope;
use crate::services::SessionStream;

mod recovery_effect;
use recovery_effect::RecoveryEffect;

const LEASE_DURATION: DurationMs = DurationMs::from_millis(30_000);

/// Drains the durable global feed for child plans and completed children.
///
/// A broker notification may call this method, but the notification carries no
/// authority: every decision is rebuilt from the event store and progress is
/// recorded only by the named cursor after the parent effect succeeds.
pub struct RecoverProtocolChildrenUseCase {
    events: Arc<dyn ProtocolEventStorePort>,
    cursors: Arc<dyn ProtocolEventCursorPort>,
    stream: Arc<SessionStream>,
    prepare: Arc<PrepareProtocolChildrenUseCase>,
    accept: Arc<AcceptChildCompletionUseCase>,
    clock: Arc<dyn ClockPort>,
    consumer: ProtocolEventConsumer,
    authorization_continuation: Option<Arc<ContinueAcceptedProtocolWorkUseCase>>,
}

impl RecoverProtocolChildrenUseCase {
    #[must_use]
    pub fn new(
        events: Arc<dyn ProtocolEventStorePort>,
        cursors: Arc<dyn ProtocolEventCursorPort>,
        stream: Arc<SessionStream>,
        prepare: Arc<PrepareProtocolChildrenUseCase>,
        accept: Arc<AcceptChildCompletionUseCase>,
        clock: Arc<dyn ClockPort>,
        consumer: ProtocolEventConsumer,
    ) -> Self {
        Self {
            events,
            cursors,
            stream,
            prepare,
            accept,
            clock,
            consumer,
            authorization_continuation: None,
        }
    }

    #[must_use]
    pub fn with_authorization_continuation(
        mut self,
        continuation: Arc<ContinueAcceptedProtocolWorkUseCase>,
    ) -> Self {
        self.authorization_continuation = Some(continuation);
        self
    }

    pub async fn execute(
        &self,
        limit: ProtocolEventPageLimit,
    ) -> Result<RecoverProtocolChildrenRound, DomainError> {
        let mut round = RecoverProtocolChildrenRound::default();
        let one = ProtocolEventPageLimit::new(1)?;
        for _ in 0..limit.value() {
            let lease_id = ProtocolEventCursorLeaseId::new(uuid::Uuid::new_v4().to_string())?;
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
                    self.release_after_error(&lease, "read global protocol events")
                        .await;
                    return Err(error);
                }
            };
            let Some(positioned) = page.into_iter().next() else {
                self.cursors.release(&lease).await?;
                break;
            };

            let effect = Box::pin(self.process(&positioned.record, lease.lease_id())).await;
            match effect {
                Ok(RecoveryEffect::PlanRecovered) => round.recovered_plans += 1,
                Ok(RecoveryEffect::CompletionAccepted) => round.accepted_completions += 1,
                Ok(RecoveryEffect::Skipped) => round.skipped += 1,
                Err(error) => {
                    if let Err(cursor_error) =
                        self.cursors.mark_failed(&lease, positioned.position).await
                    {
                        self.release_after_error(&lease, "mark protocol recovery failed")
                            .await;
                        return Err(cursor_error);
                    }
                    round.failed += 1;
                    tracing::warn!(
                        position = positioned.position.value(),
                        error = %error,
                        "child protocol recovery left the cursor pending"
                    );
                    break;
                }
            }
            if let Err(error) = self
                .cursors
                .acknowledge_lease(&lease, positioned.position)
                .await
            {
                self.release_after_error(&lease, "acknowledge protocol recovery")
                    .await;
                return Err(error);
            }
        }
        Ok(round)
    }

    async fn release_after_error(
        &self,
        lease: &made_core::value_objects::ProtocolEventCursorLease,
        operation: &'static str,
    ) {
        if let Err(error) = self.cursors.release(lease).await {
            tracing::warn!(%error, operation, "child protocol recovery could not release its cursor lease");
        }
    }

    async fn process(
        &self,
        record: &made_core::entities::AuditRecord,
        lease_id: &ProtocolEventCursorLeaseId,
    ) -> Result<RecoveryEffect, DomainError> {
        // A root completion has no child-recovery effect. Classify it before
        // asking for an accepted-work continuation: historical root events
        // predate authorization evidence and must remain readable unchanged.
        if matches!(record.event(), Some(ProtocolEvent::ProtocolCompleted(_)))
            && self
                .stream
                .load(record.protocol_id())
                .await?
                .instance
                .lineage()
                .is_none()
        {
            return Ok(RecoveryEffect::Skipped);
        }
        if let Some(continuation) = self.authorization_continuation.as_ref().filter(|_| {
            crate::services::AuthorizationOperationScope::current().is_none()
                && recovery_requires_authorization(record)
        }) {
            let request_id = made_core::value_objects::AuthorizationRequestId::new(format!(
                "protocol-recovery:{}:{}",
                record.event_id().as_str(),
                lease_id.as_str()
            ))?;
            let operation = continuation.execute(record, request_id).await?;
            return Box::pin(AuthorizationOperationScope::run(
                operation,
                self.process_authorized(record),
            ))
            .await;
        }
        self.process_authorized(record).await
    }

    async fn process_authorized(
        &self,
        record: &made_core::entities::AuditRecord,
    ) -> Result<RecoveryEffect, DomainError> {
        match record.event() {
            Some(ProtocolEvent::ChildSpawnPlanned(event)) => {
                self.prepare
                    .recover_group(record.protocol_id(), event.plan.group_id())
                    .await?;
                Ok(RecoveryEffect::PlanRecovered)
            }
            Some(ProtocolEvent::ChildSpawnPlanAdopted(event)) => {
                self.prepare
                    .recover_group(record.protocol_id(), &event.group_id)
                    .await?;
                Ok(RecoveryEffect::PlanRecovered)
            }
            Some(ProtocolEvent::ProtocolCompleted(_)) => {
                let child = self.stream.load(record.protocol_id()).await?.instance;
                if child.lineage().is_none() {
                    return Ok(RecoveryEffect::Skipped);
                }
                self.accept
                    .execute(AcceptChildCompletionInput::new(
                        record.protocol_id().clone(),
                        record.event_id().clone(),
                    ))
                    .await?;
                Ok(RecoveryEffect::CompletionAccepted)
            }
            _ => Ok(RecoveryEffect::Skipped),
        }
    }

    #[must_use]
    pub fn consumer(&self) -> &ProtocolEventConsumer {
        &self.consumer
    }
}

fn recovery_requires_authorization(record: &made_core::entities::AuditRecord) -> bool {
    matches!(
        record.event(),
        Some(
            ProtocolEvent::ChildSpawnPlanned(_)
                | ProtocolEvent::ChildSpawnPlanAdopted(_)
                | ProtocolEvent::ProtocolCompleted(_)
        )
    )
}

impl std::fmt::Debug for RecoverProtocolChildrenUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RecoverProtocolChildrenUseCase")
            .field("consumer", &self.consumer)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;
    use made_core::entities::AuditFact;
    use made_core::ports::{AppendOutcome, NoopProtocolEventSubscriber, ProtocolEventCursorPort};
    use made_core::value_objects::{
        AuditActorKind, GlobalPosition, IdempotencyKey, LeaseOwnerId, ProtocolEventCursorAttempt,
        ProtocolEventQuarantineReason, QuarantinedProtocolEvent, StreamVersion,
    };
    use tokio::sync::Mutex;

    use super::*;
    use crate::usecases::protocol_test_support::{
        a_memory, child_spawning_definition, lease_ttl, now, protocol_id, resolver_with,
        review_child_definition, role_id, started_instance, step_id, EventStoreFake, FixedClock,
        PublicationsFake, StepHandlerFake,
    };
    use crate::usecases::{RunProtocolStepInput, RunProtocolStepUseCase};

    struct FailFirstChildOpening {
        store: Arc<EventStoreFake>,
        parent_id: made_core::value_objects::ProtocolId,
        failures_remaining: AtomicUsize,
    }

    struct FailFirstGlobalRead {
        store: Arc<EventStoreFake>,
        failures_remaining: AtomicUsize,
    }

    #[async_trait]
    impl ProtocolEventStorePort for FailFirstChildOpening {
        async fn append(
            &self,
            stream: &made_core::value_objects::ProtocolId,
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
            stream: &made_core::value_objects::ProtocolId,
            after: StreamVersion,
            limit: ProtocolEventPageLimit,
        ) -> Result<Vec<made_core::entities::AuditRecord>, DomainError> {
            self.store.read(stream, after, limit).await
        }

        async fn read_all(
            &self,
            from: GlobalPosition,
            limit: ProtocolEventPageLimit,
        ) -> Result<Vec<made_core::ports::PositionedRecord>, DomainError> {
            self.store.read_all(from, limit).await
        }

        async fn head(
            &self,
            stream: &made_core::value_objects::ProtocolId,
        ) -> Result<StreamVersion, DomainError> {
            self.store.head(stream).await
        }

        async fn streams(&self) -> Result<Vec<made_core::value_objects::ProtocolId>, DomainError> {
            self.store.streams().await
        }
    }

    #[async_trait]
    impl ProtocolEventStorePort for FailFirstGlobalRead {
        async fn append(
            &self,
            stream: &made_core::value_objects::ProtocolId,
            expected: StreamVersion,
            facts: Vec<AuditFact>,
        ) -> Result<AppendOutcome, DomainError> {
            self.store.append(stream, expected, facts).await
        }

        async fn read(
            &self,
            stream: &made_core::value_objects::ProtocolId,
            after: StreamVersion,
            limit: ProtocolEventPageLimit,
        ) -> Result<Vec<made_core::entities::AuditRecord>, DomainError> {
            self.store.read(stream, after, limit).await
        }

        async fn read_all(
            &self,
            from: GlobalPosition,
            limit: ProtocolEventPageLimit,
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
            stream: &made_core::value_objects::ProtocolId,
        ) -> Result<StreamVersion, DomainError> {
            self.store.head(stream).await
        }

        async fn streams(&self) -> Result<Vec<made_core::value_objects::ProtocolId>, DomainError> {
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
        lease: Option<made_core::value_objects::ProtocolEventCursorLease>,
        attempt: ProtocolEventCursorAttempt,
    }

    #[async_trait]
    impl ProtocolEventCursorPort for TestCursor {
        async fn position(
            &self,
            _consumer: &ProtocolEventConsumer,
        ) -> Result<Option<GlobalPosition>, DomainError> {
            Ok(self.state.lock().await.position)
        }

        async fn lease(
            &self,
            consumer: &ProtocolEventConsumer,
            lease_id: ProtocolEventCursorLeaseId,
            now: time::OffsetDateTime,
            duration: DurationMs,
        ) -> Result<Option<made_core::value_objects::ProtocolEventCursorLease>, DomainError>
        {
            let mut state = self.state.lock().await;
            if state.lease.is_some() {
                return Ok(None);
            }
            let lease = made_core::value_objects::ProtocolEventCursorLease::new(
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
            _consumer: &ProtocolEventConsumer,
            through: GlobalPosition,
        ) -> Result<(), DomainError> {
            let mut state = self.state.lock().await;
            state.position = Some(through);
            state.attempt = ProtocolEventCursorAttempt::NONE;
            Ok(())
        }

        async fn acknowledge_lease(
            &self,
            lease: &made_core::value_objects::ProtocolEventCursorLease,
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
            state.attempt = ProtocolEventCursorAttempt::NONE;
            state.lease = None;
            Ok(())
        }

        async fn mark_failed(
            &self,
            _lease: &made_core::value_objects::ProtocolEventCursorLease,
            _position: GlobalPosition,
        ) -> Result<(), DomainError> {
            let mut state = self.state.lock().await;
            state.attempt = state.attempt.next();
            state.lease = None;
            Ok(())
        }

        async fn quarantine(
            &self,
            _lease: &made_core::value_objects::ProtocolEventCursorLease,
            _position: GlobalPosition,
            _reason: ProtocolEventQuarantineReason,
            _now: time::OffsetDateTime,
        ) -> Result<(), DomainError> {
            Err(DomainError::InvariantViolated {
                reason: "test cursor does not quarantine",
            })
        }

        async fn release(
            &self,
            _lease: &made_core::value_objects::ProtocolEventCursorLease,
        ) -> Result<(), DomainError> {
            self.state.lock().await.lease = None;
            Ok(())
        }

        async fn quarantined(
            &self,
            _consumer: &ProtocolEventConsumer,
        ) -> Result<Vec<QuarantinedProtocolEvent>, DomainError> {
            Ok(Vec::new())
        }
    }

    #[tokio::test]
    async fn a_transient_feed_read_releases_the_lease_for_the_next_tick() {
        let definition = child_spawning_definition();
        let definitions = Arc::new(
            crate::usecases::protocol_test_support::DefinitionRepositoryFake::new(
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
            Arc::new(NoopProtocolEventSubscriber),
        ));
        let resolver = resolver_with(definitions, publications.clone());
        let prepare = Arc::new(PrepareProtocolChildrenUseCase::new(
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
        let recover = RecoverProtocolChildrenUseCase::new(
            Arc::new(FailFirstGlobalRead {
                store,
                failures_remaining: AtomicUsize::new(1),
            }),
            cursor.clone(),
            stream,
            prepare,
            accept,
            Arc::new(FixedClock::new(now())),
            ProtocolEventConsumer::new("children-transient-read").unwrap(),
        );

        let first = recover
            .execute(ProtocolEventPageLimit::new(1).unwrap())
            .await
            .unwrap_err();
        assert!(first.to_string().contains("transient global read"));

        let second = recover
            .execute(ProtocolEventPageLimit::new(1).unwrap())
            .await
            .unwrap();
        assert!(!second.busy);
        assert_eq!(second.skipped, 1);
        assert!(cursor.position(recover.consumer()).await.unwrap().is_some());
    }

    async fn fail_before_first_child_open(
        store: Arc<EventStoreFake>,
        resolver: Arc<crate::usecases::ResolveProtocolDefinitionUseCase>,
        publications: Arc<PublicationsFake>,
        at: time::OffsetDateTime,
        worker: &str,
        idempotency_key: &str,
    ) -> DomainError {
        let stream = Arc::new(SessionStream::new(
            Arc::new(FailFirstChildOpening {
                store: store.clone(),
                parent_id: protocol_id(),
                failures_remaining: AtomicUsize::new(1),
            }),
            store,
            Arc::new(NoopProtocolEventSubscriber),
        ));
        let prepare = Arc::new(PrepareProtocolChildrenUseCase::new(
            resolver.clone(),
            publications,
            stream.clone(),
            Arc::new(FixedClock::new(at)),
            a_memory(),
        ));
        RunProtocolStepUseCase::new(
            resolver,
            stream,
            Arc::new(StepHandlerFake::failing(DomainError::InvariantViolated {
                reason: "spawn handler must not run",
            })),
            Arc::new(FixedClock::new(at)),
        )
        .with_child_orchestrator(prepare)
        .execute(RunProtocolStepInput::new(
            protocol_id(),
            role_id(),
            AuditActorKind::Agent,
            step_id(),
            LeaseOwnerId::new(worker).unwrap(),
            IdempotencyKey::new(idempotency_key).unwrap(),
            lease_ttl(),
        ))
        .await
        .unwrap_err()
    }

    async fn recover_adopted_plan(
        store: Arc<EventStoreFake>,
        resolver: Arc<crate::usecases::ResolveProtocolDefinitionUseCase>,
        publications: Arc<PublicationsFake>,
        at: time::OffsetDateTime,
        child_id: &made_core::value_objects::ProtocolId,
    ) {
        let plan_position = store
            .read_all(GlobalPosition::FIRST, ProtocolEventPageLimit::DEFAULT)
            .await
            .unwrap()
            .into_iter()
            .find(|positioned| {
                matches!(
                    positioned.record.event(),
                    Some(ProtocolEvent::ChildSpawnPlanned(_))
                )
            })
            .unwrap()
            .position;
        let consumer = ProtocolEventConsumer::new("children-test").unwrap();
        let cursor = Arc::new(TestCursor::default());
        cursor.acknowledge(&consumer, plan_position).await.unwrap();
        let stream = Arc::new(SessionStream::new(
            store.clone(),
            store.clone(),
            Arc::new(NoopProtocolEventSubscriber),
        ));
        let prepare = Arc::new(PrepareProtocolChildrenUseCase::new(
            resolver.clone(),
            publications.clone(),
            stream.clone(),
            Arc::new(FixedClock::new(at)),
            a_memory(),
        ));
        let accept = Arc::new(AcceptChildCompletionUseCase::new(
            resolver,
            publications,
            stream.clone(),
            Arc::new(FixedClock::new(at)),
        ));
        let recover = RecoverProtocolChildrenUseCase::new(
            store.clone(),
            cursor,
            stream,
            prepare,
            accept,
            Arc::new(FixedClock::new(at)),
            consumer,
        );

        let round = recover
            .execute(ProtocolEventPageLimit::DEFAULT)
            .await
            .unwrap();
        assert!(round.recovered_plans >= 1, "the adoption wake was ignored");
        assert!(store.exists(child_id).await);
        assert_eq!(
            store
                .saved(&protocol_id())
                .await
                .step_record(&step_id())
                .unwrap()
                .status(),
            made_core::value_objects::StepStatus::Completed
        );
    }

    #[tokio::test]
    async fn an_adoption_wake_recovers_after_a_crash_before_the_first_child_open() {
        let definition = child_spawning_definition();
        let definitions = Arc::new(
            crate::usecases::protocol_test_support::DefinitionRepositoryFake::new(
                definition.clone(),
            ),
        );
        let publications = Arc::new(PublicationsFake::default());
        publications.seed(review_child_definition()).await;
        let store = Arc::new(EventStoreFake::default());
        store.save(&started_instance(&definition)).await.unwrap();
        let resolver = resolver_with(definitions.clone(), publications.clone());
        let first_error = fail_before_first_child_open(
            store.clone(),
            resolver.clone(),
            publications.clone(),
            now(),
            "crashed-worker",
            "crash-after-plan",
        )
        .await;
        assert!(first_error.to_string().contains("before child opening"));

        let later = now() + time::Duration::seconds(120);
        let adoption_error = fail_before_first_child_open(
            store.clone(),
            resolver.clone(),
            publications.clone(),
            later,
            "replacement-worker",
            "crash-after-adoption",
        )
        .await;
        assert!(
            adoption_error.to_string().contains("before child opening"),
            "unexpected reclaim failure: {adoption_error}"
        );
        let before = store.saved(&protocol_id()).await;
        let group = before.child_groups().values().next().unwrap();
        let child_id = group.plan().children()[0].child_id().clone();
        assert!(!store.exists(&child_id).await);

        recover_adopted_plan(store.clone(), resolver, publications, later, &child_id).await;
        let records = store.records(&protocol_id()).await;
        assert_eq!(
            records
                .iter()
                .filter(|record| matches!(
                    record.event(),
                    Some(ProtocolEvent::ChildSpawnPlanned(_))
                ))
                .count(),
            1
        );
        assert_eq!(
            records
                .iter()
                .filter(|record| matches!(
                    record.event(),
                    Some(ProtocolEvent::ChildSpawnPlanAdopted(_))
                ))
                .count(),
            1
        );
    }
}
