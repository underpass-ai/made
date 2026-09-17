//! [`StartCeremonyStepUseCase`] — acquire a lease for a ceremony step.

use std::sync::Arc;

use made_core::entities::ceremony_commands::StartStep;
use made_core::entities::CeremonyCommand;
use made_core::error::DomainError;
use made_core::ports::ClockPort;
use made_core::value_objects::{MaxParallel, StepAttempt, StepLease};

use super::resolve_ceremony_definition_use_case::ResolveCeremonyDefinitionUseCase;
use super::start_ceremony_step_input::StartCeremonyStepInput;
use crate::services::{session_facts, ConflictPolicy, SessionStream};

pub struct StartCeremonyStepUseCase {
    definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    stream: Arc<SessionStream>,
    clock: Arc<dyn ClockPort>,
    max_parallel_ceiling: MaxParallel,
}

impl std::fmt::Debug for StartCeremonyStepUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StartCeremonyStepUseCase").finish()
    }
}

impl StartCeremonyStepUseCase {
    #[must_use]
    pub fn new(
        definitions: Arc<ResolveCeremonyDefinitionUseCase>,
        stream: Arc<SessionStream>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            definitions,
            stream,
            clock,
            max_parallel_ceiling: MaxParallel::SERVER_MAX,
        }
    }

    #[must_use]
    pub fn with_max_parallel_ceiling(mut self, ceiling: MaxParallel) -> Self {
        self.max_parallel_ceiling = ceiling;
        self
    }

    #[tracing::instrument(
        name = "start_ceremony_step",
        skip_all,
        fields(ceremony_id = %input.instance_id, step_id = %input.step_id)
    )]
    pub async fn execute(&self, input: StartCeremonyStepInput) -> Result<StepAttempt, DomainError> {
        let session = self.stream.load(&input.instance_id).await?;
        // Resolved from the instance, never from the request: a session
        // bound to a published version must be advanced by the very
        // definition it recorded, and one that is unbound has only the
        // repository to go to. Reading coordinates off the caller made
        // a bound session unadvanceable, because publishing writes to
        // the catalogue and not to the repository.
        let definition = self.definitions.execute(&session.instance).await?;
        let actor = session_facts::seat(&input.role_id, input.role_kind)?;
        let now = self.clock.now();
        let lease = StepLease::acquire(
            input.lease_owner_id,
            input.idempotency_key,
            now,
            input.lease_ttl,
        )?;
        let command = CeremonyCommand::StartStep(StartStep {
            role_id: Some(input.role_id),
            step_id: input.step_id.clone(),
            lease,
            now,
            max_parallel_ceiling: self.max_parallel_ceiling,
        });
        // The claim commutes with what other writers do to the session
        // — a second claim of the same step is refused by the lease,
        // not by the store — so a lost race is decided again.
        let session = self
            .stream
            .execute(session, ConflictPolicy::retry(), |session| {
                let events = session.instance.decide(&command, &definition)?;
                session_facts::facts(&session.instance, events, &actor, now)
            })
            .await?;
        Ok(session
            .instance
            .step_record(&input.step_id)
            .ok_or(DomainError::NotFound {
                what: "ceremony_step",
            })?
            .attempt())
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use made_core::entities::{AuditFact, AuditRecord, CeremonyDefinition, CeremonyEvent};
    use std::sync::Arc;

    use made_core::error::DomainError;
    use made_core::ports::{
        AppendOutcome, CeremonyEventStorePort, NoopCeremonyEventSubscriber, PositionedRecord,
    };
    use made_core::value_objects::{
        AuditActorKind, AuditEventType, CeremonyEventPageLimit, CeremonyGuard, CeremonyName,
        CeremonyRole, CeremonyState, CeremonyStep, CeremonyTransition, CeremonyVersion,
        GlobalPosition, GuardCondition, GuardName, MaxParallel, RetryPolicy, RoleAction, RoleId,
        StateExecution, StateId, StepHandlerConfig, StepHandlerKind, StepId, StepStatus,
        StreamVersion, TransitionTrigger,
    };
    use time::Duration;
    use tokio::sync::Barrier;

    use super::*;
    use crate::usecases::ceremony_test_support::{
        ceremony_id, definition, definition_resolver, idempotency_key, lease_owner, lease_ttl, now,
        role_id, started_instance, step_id, stream, stream_over, DefinitionRepositoryFake,
        EventStoreFake, FixedClock,
    };

    #[tokio::test]
    async fn acquires_step_lease_and_persists_in_progress_record() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        instances
            .save(&started_instance(&definition))
            .await
            .unwrap();
        let usecase = StartCeremonyStepUseCase::new(
            definition_resolver(definitions),
            stream(instances.clone()),
            Arc::new(FixedClock::new(now())),
        );

        let attempt = usecase
            .execute(StartCeremonyStepInput::new(
                ceremony_id(),
                role_id(),
                AuditActorKind::Agent,
                step_id(),
                lease_owner(),
                idempotency_key("lease-1"),
                lease_ttl(),
            ))
            .await
            .unwrap();

        assert_eq!(attempt, StepAttempt::FIRST);
        let saved = instances.saved(&ceremony_id()).await;
        let record = saved.step_record(&step_id()).unwrap();
        assert_eq!(record.status(), StepStatus::InProgress);
        assert_eq!(record.lease().unwrap().owner_id().as_str(), "runner-1");
    }

    #[tokio::test]
    async fn active_lease_blocks_second_runner() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        instances
            .save(&started_instance(&definition))
            .await
            .unwrap();
        let usecase = StartCeremonyStepUseCase::new(
            definition_resolver(definitions),
            stream(instances),
            Arc::new(FixedClock::new(now())),
        );
        usecase
            .execute(StartCeremonyStepInput::new(
                ceremony_id(),
                role_id(),
                AuditActorKind::Agent,
                step_id(),
                lease_owner(),
                idempotency_key("lease-1"),
                lease_ttl(),
            ))
            .await
            .unwrap();

        let err = usecase
            .execute(StartCeremonyStepInput::new(
                ceremony_id(),
                role_id(),
                AuditActorKind::Agent,
                step_id(),
                lease_owner(),
                idempotency_key("lease-2"),
                lease_ttl(),
            ))
            .await
            .unwrap_err();

        assert!(matches!(err, DomainError::InvariantViolated { .. }));
    }

    #[tokio::test]
    async fn expired_lease_allows_failover_attempt() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        instances
            .save(&started_instance(&definition))
            .await
            .unwrap();
        let first = StartCeremonyStepUseCase::new(
            definition_resolver(definitions.clone()),
            stream(instances.clone()),
            Arc::new(FixedClock::new(now())),
        );
        first
            .execute(StartCeremonyStepInput::new(
                ceremony_id(),
                role_id(),
                AuditActorKind::Agent,
                step_id(),
                lease_owner(),
                idempotency_key("lease-1"),
                lease_ttl(),
            ))
            .await
            .unwrap();
        let second = StartCeremonyStepUseCase::new(
            definition_resolver(definitions),
            stream(instances),
            Arc::new(FixedClock::new(now() + Duration::seconds(61))),
        );

        let attempt = second
            .execute(StartCeremonyStepInput::new(
                ceremony_id(),
                role_id(),
                AuditActorKind::Agent,
                step_id(),
                lease_owner(),
                idempotency_key("lease-2"),
                lease_ttl(),
            ))
            .await
            .unwrap();

        assert_eq!(attempt, StepAttempt::new(2).unwrap());
    }

    /// Claiming a step without running it still leaves a record.
    ///
    /// This path exists for hosts that execute the work themselves, so
    /// the engine sees the claim and never the ending. A claim that
    /// sealed nothing would make those sessions look like nobody
    /// touched them.
    #[tokio::test]
    async fn seals_the_claim_even_though_nothing_ran() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        instances
            .save(&started_instance(&definition))
            .await
            .unwrap();
        let (stream, store) = stream_over(instances);
        let usecase = StartCeremonyStepUseCase::new(
            definition_resolver(definitions),
            stream,
            Arc::new(FixedClock::new(now())),
        );

        usecase
            .execute(StartCeremonyStepInput::new(
                ceremony_id(),
                role_id(),
                AuditActorKind::Human,
                step_id(),
                lease_owner(),
                idempotency_key("claim-1"),
                lease_ttl(),
            ))
            .await
            .unwrap();

        let facts = store.facts().await;
        assert_eq!(facts.len(), 1, "one claim, one fact: {facts:?}");
        assert_eq!(facts[0].event.event_type(), AuditEventType::StepStarted);
        let CeremonyEvent::StepStarted(claimed) = &facts[0].event else {
            panic!("a claim seals its lease: {:?}", facts[0].event);
        };
        assert_eq!(claimed.step_id, step_id());
        assert_eq!(claimed.lease.idempotency_key(), &idempotency_key("claim-1"));
        assert_eq!(claimed.started_by, role_id());
        assert_eq!(facts[0].actor.kind(), AuditActorKind::Human);
    }

    #[tokio::test]
    async fn simultaneous_claims_recheck_capacity_after_the_winning_append() {
        let definition = one_slot_concurrent_definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        instances
            .save(&started_instance(&definition))
            .await
            .unwrap();
        let events = Arc::new(ClaimsMeetAtAppend {
            inner: instances.clone(),
            barrier: Barrier::new(2),
        });
        let stream = Arc::new(SessionStream::new(
            events,
            instances.clone(),
            Arc::new(NoopCeremonyEventSubscriber),
        ));
        let first = StartCeremonyStepUseCase::new(
            definition_resolver(definitions.clone()),
            stream.clone(),
            Arc::new(FixedClock::new(now())),
        );
        let second = StartCeremonyStepUseCase::new(
            definition_resolver(definitions),
            stream,
            Arc::new(FixedClock::new(now())),
        );

        let (a, b) = tokio::join!(
            first.execute(StartCeremonyStepInput::new(
                ceremony_id(),
                RoleId::new("A").unwrap(),
                AuditActorKind::Agent,
                StepId::new("a").unwrap(),
                lease_owner(),
                idempotency_key("parallel-a"),
                lease_ttl(),
            )),
            second.execute(StartCeremonyStepInput::new(
                ceremony_id(),
                RoleId::new("B").unwrap(),
                AuditActorKind::Agent,
                StepId::new("b").unwrap(),
                lease_owner(),
                idempotency_key("parallel-b"),
                lease_ttl(),
            )),
        );

        assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
        let refusal = a.err().or_else(|| b.err()).unwrap();
        assert!(matches!(refusal, DomainError::InvariantViolated { .. }));
        let saved = instances.saved(&ceremony_id()).await;
        assert_eq!(
            saved
                .step_records()
                .values()
                .filter(|record| record.status() == StepStatus::InProgress)
                .count(),
            1
        );
        assert_eq!(instances.records(&ceremony_id()).await.len(), 2);
    }

    #[derive(Debug)]
    struct ClaimsMeetAtAppend {
        inner: Arc<EventStoreFake>,
        barrier: Barrier,
    }

    #[async_trait]
    impl CeremonyEventStorePort for ClaimsMeetAtAppend {
        async fn append(
            &self,
            stream: &made_core::value_objects::CeremonyId,
            expected: StreamVersion,
            facts: Vec<AuditFact>,
        ) -> Result<AppendOutcome, DomainError> {
            if expected == StreamVersion::new(1) {
                self.barrier.wait().await;
            }
            self.inner.append(stream, expected, facts).await
        }

        async fn read(
            &self,
            stream: &made_core::value_objects::CeremonyId,
            after: StreamVersion,
            limit: CeremonyEventPageLimit,
        ) -> Result<Vec<AuditRecord>, DomainError> {
            self.inner.read(stream, after, limit).await
        }

        async fn read_all(
            &self,
            from: GlobalPosition,
            limit: CeremonyEventPageLimit,
        ) -> Result<Vec<PositionedRecord>, DomainError> {
            self.inner.read_all(from, limit).await
        }

        async fn head(
            &self,
            stream: &made_core::value_objects::CeremonyId,
        ) -> Result<StreamVersion, DomainError> {
            self.inner.head(stream).await
        }

        async fn streams(&self) -> Result<Vec<made_core::value_objects::CeremonyId>, DomainError> {
            self.inner.streams().await
        }
    }

    fn one_slot_concurrent_definition() -> CeremonyDefinition {
        let work = StateId::new("WORK").unwrap();
        let done = StateId::new("DONE").unwrap();
        let finish = TransitionTrigger::new("finish").unwrap();
        let join = CeremonyGuard::new(
            GuardName::new("one_done").unwrap(),
            GuardCondition::AnyStepCompleted,
        );
        let steps = ["a", "b"]
            .into_iter()
            .map(|id| {
                CeremonyStep::new(
                    StepId::new(id).unwrap(),
                    work.clone(),
                    StepHandlerKind::new("host_callback").unwrap(),
                    StepHandlerConfig::empty(),
                    RetryPolicy::single_attempt(),
                    None,
                )
            })
            .collect::<Vec<_>>();
        let roles = ["A", "B"]
            .into_iter()
            .zip(["a", "b"])
            .map(|(role, step)| {
                let mut actions = vec![RoleAction::step(StepId::new(step).unwrap())];
                if role == "A" {
                    actions.push(RoleAction::transition(finish.clone()));
                }
                CeremonyRole::new(RoleId::new(role).unwrap(), actions).unwrap()
            })
            .collect::<Vec<_>>();
        CeremonyDefinition::new(
            CeremonyName::new("one_slot_concurrent").unwrap(),
            CeremonyVersion::v1(),
            None,
            Vec::new(),
            Vec::new(),
            vec![
                CeremonyState::initial(work.clone()).with_execution(StateExecution::Concurrent),
                CeremonyState::terminal(done.clone()),
            ],
            vec![CeremonyTransition::new(work, done, finish, vec![join.name().clone()]).unwrap()],
            steps,
            vec![join],
            roles,
        )
        .unwrap()
        .with_max_parallel(MaxParallel::new(1).unwrap())
    }
}
