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
    use made_core::entities::CeremonyEvent;
    use std::sync::Arc;

    use made_core::error::DomainError;
    use made_core::value_objects::{AuditActorKind, AuditEventType, StepStatus};
    use time::Duration;

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
}
