//! [`CompleteCeremonyStepUseCase`] — apply a step result to a ceremony.

use std::sync::Arc;

use made_core::entities::ceremony_commands::ApplyStepResult;
use made_core::entities::{CeremonyCommand, CeremonyInstance};
use made_core::error::DomainError;
use made_core::ports::ClockPort;

use super::complete_ceremony_step_input::CompleteCeremonyStepInput;
use super::resolve_ceremony_definition_use_case::ResolveCeremonyDefinitionUseCase;
use crate::services::{session_facts, ConflictPolicy, SessionStream};

pub struct CompleteCeremonyStepUseCase {
    definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    stream: Arc<SessionStream>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for CompleteCeremonyStepUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompleteCeremonyStepUseCase").finish()
    }
}

impl CompleteCeremonyStepUseCase {
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
        }
    }

    #[tracing::instrument(
        name = "complete_ceremony_step",
        skip_all,
        fields(ceremony_id = %input.instance_id, step_id = %input.step_id)
    )]
    pub async fn execute(
        &self,
        input: CompleteCeremonyStepInput,
    ) -> Result<CeremonyInstance, DomainError> {
        let session = self.stream.load(&input.instance_id).await?;
        // Resolved from the instance, never from the request: a session
        // bound to a published version must be advanced by the very
        // definition it recorded, and one that is unbound has only the
        // repository to go to. Reading coordinates off the caller made
        // a bound session unadvanceable, because publishing writes to
        // the catalogue and not to the repository.
        let definition = self.definitions.execute(&session.instance).await?;
        // The seat is the definition's to say, as it is everywhere a
        // step is run. Only what filled it had to be declared.
        let finished_by = definition.role_id_for_step(&input.step_id)?;
        let actor = session_facts::seat(&finished_by, input.actor_kind)?;
        let now = self.clock.now();
        let command = CeremonyCommand::ApplyStepResult(ApplyStepResult {
            step_id: input.step_id,
            result: input.result,
            now,
        });
        // A step ending commutes with what other writers do to the
        // session, so a lost race is decided again; a step that was
        // ended meanwhile is refused by the decision, not the store.
        self.stream
            .execute(session, ConflictPolicy::retry(), |session| {
                let events = session.instance.decide(&command, &definition)?;
                session_facts::facts(&session.instance, events, &actor, now)
            })
            .await
            .map(|session| session.instance)
    }
}

#[cfg(test)]
mod tests {
    use made_core::entities::CeremonyEvent;
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use made_core::value_objects::{
        Attributes, AuditActorKind, AuditEventType, StepOutput, StepResult, StepStatus,
    };

    use super::*;
    use crate::usecases::ceremony_test_support::{
        ceremony_id, definition, definition_resolver, idempotency_key, lease_owner, now,
        repeating_definition, role_id, started_instance, step_id, stream, stream_conflicting_once,
        stream_over, DefinitionRepositoryFake, EventStoreFake, FixedClock,
    };

    fn readiness_output(ready: bool) -> StepOutput {
        StepOutput::new(
            Attributes::new(BTreeMap::from([(
                "ready".to_owned(),
                serde_json::json!(ready),
            )]))
            .unwrap(),
        )
    }

    /// A step ending commutes with what other writers do, so one lost
    /// race is decided again against the reloaded fold and lands on
    /// the second attempt — once, not twice: the stream holds exactly
    /// one ending for the step.
    #[tokio::test]
    async fn a_step_ending_that_lost_one_race_lands_on_the_second_attempt() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        let mut instance = started_instance(&definition);
        let lease = made_core::value_objects::StepLease::new(
            lease_owner(),
            idempotency_key("lease-1"),
            now(),
            now() + time::Duration::seconds(60),
        )
        .unwrap();
        instance
            .start_step_as(&definition, &role_id(), &step_id(), lease, now())
            .unwrap();
        instances.save(&instance).await.unwrap();
        let usecase = CompleteCeremonyStepUseCase::new(
            definition_resolver(definitions),
            stream_conflicting_once(instances.clone()),
            Arc::new(FixedClock::new(now())),
        );

        let completed = usecase
            .execute(CompleteCeremonyStepInput::new(
                ceremony_id(),
                step_id(),
                StepResult::completed(StepOutput::empty()).unwrap(),
                AuditActorKind::Agent,
            ))
            .await
            .expect("a commuting command rides out one lost race");

        assert_eq!(
            completed.step_record(&step_id()).unwrap().status(),
            StepStatus::Completed
        );
        let endings = instances
            .records(&ceremony_id())
            .await
            .into_iter()
            .filter(|record| record.event_type() == AuditEventType::StepCompleted)
            .count();
        assert_eq!(endings, 1, "the retry must not seal the ending twice");
        assert_eq!(
            instances
                .saved(&ceremony_id())
                .await
                .step_record(&step_id())
                .unwrap()
                .status(),
            StepStatus::Completed
        );
    }

    #[tokio::test]
    async fn applies_step_result_and_clears_lease() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        let mut instance = started_instance(&definition);
        let lease = made_core::value_objects::StepLease::new(
            lease_owner(),
            idempotency_key("lease-1"),
            now(),
            now() + time::Duration::seconds(60),
        )
        .unwrap();
        instance
            .start_step_as(&definition, &role_id(), &step_id(), lease, now())
            .unwrap();
        instances.save(&instance).await.unwrap();
        let usecase = CompleteCeremonyStepUseCase::new(
            definition_resolver(definitions),
            stream(instances.clone()),
            Arc::new(FixedClock::new(now())),
        );

        let completed = usecase
            .execute(CompleteCeremonyStepInput::new(
                ceremony_id(),
                step_id(),
                StepResult::completed(StepOutput::empty()).unwrap(),
                AuditActorKind::Agent,
            ))
            .await
            .unwrap();

        let record = completed.step_record(&step_id()).unwrap();
        assert_eq!(record.status(), StepStatus::Completed);
        assert!(record.lease().is_none());
        assert_eq!(
            instances
                .saved(&ceremony_id())
                .await
                .step_record(&step_id())
                .unwrap()
                .status(),
            StepStatus::Completed
        );
    }

    /// A result reported from outside still names an attempt, and the
    /// session is what names it.
    ///
    /// This path exists for hosts that run the work themselves, so the
    /// engine never saw the step run and has only what it recorded when
    /// the step was claimed. Taking the attempt from the caller would
    /// let a late result be filed against an attempt that is no longer
    /// the one running — the retry's ending recorded under the attempt
    /// it replaced.
    #[tokio::test]
    async fn files_the_ending_under_the_attempt_the_session_recorded() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        let mut instance = started_instance(&definition);
        instance
            .start_step(
                &definition,
                &step_id(),
                made_core::value_objects::StepLease::new(
                    lease_owner(),
                    idempotency_key("lease-1"),
                    now(),
                    now() + time::Duration::seconds(60),
                )
                .unwrap(),
                now(),
            )
            .unwrap();
        instances.save(&instance).await.unwrap();
        let attempt_when_claimed = instance.step_record(&step_id()).unwrap().attempt();
        let (stream, store) = stream_over(instances);
        let usecase = CompleteCeremonyStepUseCase::new(
            definition_resolver(definitions),
            stream,
            Arc::new(FixedClock::new(now())),
        );

        usecase
            .execute(CompleteCeremonyStepInput::new(
                ceremony_id(),
                step_id(),
                StepResult::completed(StepOutput::empty()).unwrap(),
                AuditActorKind::Human,
            ))
            .await
            .unwrap();

        let facts = store.facts().await;
        assert_eq!(facts.len(), 1, "one ending, one fact: {facts:?}");
        assert_eq!(facts[0].event.event_type(), AuditEventType::StepCompleted);
        let CeremonyEvent::StepCompleted(completed) = &facts[0].event else {
            panic!("a completion seals its result: {:?}", facts[0].event);
        };
        assert_eq!(completed.step_id, step_id());
        assert!(completed.result.is_success());
        assert_eq!(completed.next_iteration, None);
        assert_eq!(facts[0].actor.kind(), AuditActorKind::Human);
        assert!(
            facts[0]
                .event_id
                .as_str()
                .contains(&format!("attempt:{}", attempt_when_claimed.get())),
            "the ending was filed under an attempt the session never claimed: {}",
            facts[0].event_id.as_str()
        );
        // The seat came from the definition, not from the caller, who
        // never named one.
        assert_eq!(
            facts[0].actor.role_id(),
            Some(&definition.role_id_for_step(&step_id()).unwrap())
        );
    }

    #[tokio::test]
    async fn delegated_completion_exposes_the_next_semantic_iteration() {
        let definition = repeating_definition(3);
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        let mut instance = started_instance(&definition);
        instance
            .start_step(
                &definition,
                &step_id(),
                made_core::value_objects::StepLease::new(
                    lease_owner(),
                    idempotency_key("delegated-repeat-1"),
                    now(),
                    now() + time::Duration::seconds(60),
                )
                .unwrap(),
                now(),
            )
            .unwrap();
        instances.save(&instance).await.unwrap();
        let (stream, store) = stream_over(instances);
        let usecase = CompleteCeremonyStepUseCase::new(
            definition_resolver(definitions),
            stream,
            Arc::new(FixedClock::new(now())),
        );

        let updated = usecase
            .execute(CompleteCeremonyStepInput::new(
                ceremony_id(),
                step_id(),
                StepResult::completed(readiness_output(false)).unwrap(),
                AuditActorKind::Agent,
            ))
            .await
            .unwrap();

        let record = updated.step_record(&step_id()).unwrap();
        assert_eq!(record.status(), StepStatus::Pending);
        assert_eq!(record.iteration().get(), 2);
        assert_eq!(updated.step_record_history(&step_id()).len(), 1);
        let facts = store.facts().await;
        assert_eq!(facts.len(), 1);
        assert!(facts[0].event_id.as_str().contains("iteration:1"));
    }
}
