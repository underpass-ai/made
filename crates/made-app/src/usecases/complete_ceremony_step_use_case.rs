//! [`CompleteCeremonyStepUseCase`] — apply a step result to a ceremony.

use std::sync::Arc;

use made_core::entities::CeremonyInstance;
use made_core::error::DomainError;
use made_core::ports::ClockPort;

use super::complete_ceremony_step_input::CompleteCeremonyStepInput;
use super::resolve_ceremony_definition_use_case::ResolveCeremonyDefinitionUseCase;
use crate::services::{session_facts, SessionJournal};

pub struct CompleteCeremonyStepUseCase {
    definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    journal: Arc<SessionJournal>,
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
        journal: Arc<SessionJournal>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            definitions,
            journal,
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
        // Loaded through the journal so the revision is read before
        // the session: the other order lets a concurrent write turn a
        // race into a silent overwrite.
        let mut session = self.journal.load(&input.instance_id).await?;
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
        let now = self.clock.now();
        let result = input.result;
        // Capture both coordinates before applying the result: a successful
        // repeat advances the current record to the next semantic iteration.
        let record = session
            .instance
            .step_record(&input.step_id)
            .ok_or(DomainError::NotFound {
                what: "ceremony_step",
            })?;
        let iteration = record.iteration();
        let attempt = record.attempt();
        session
            .instance
            .apply_step_result(&definition, &input.step_id, result.clone(), now)?;
        let fact = session_facts::step_finished(
            &session.instance,
            &input.step_id,
            iteration,
            attempt,
            &result,
            &finished_by,
            input.actor_kind,
            now,
        )?;
        self.journal
            .commit(session, vec![fact])
            .await
            .map(|session| session.instance)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use made_core::ports::CeremonyInstanceRepositoryPort;
    use made_core::value_objects::{
        Attributes, AuditActorKind, AuditEventType, StepOutput, StepResult, StepStatus,
    };

    use super::*;
    use crate::usecases::ceremony_test_support::{
        ceremony_id, definition, definition_resolver, idempotency_key, journal, journal_over,
        lease_owner, now, repeating_definition, role_id, started_instance, step_id,
        DefinitionRepositoryFake, FixedClock, InstanceRepositoryFake,
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

    #[tokio::test]
    async fn applies_step_result_and_clears_lease() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(InstanceRepositoryFake::default());
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
            journal(instances.clone()),
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
        let instances = Arc::new(InstanceRepositoryFake::default());
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
        let (journal, unit_of_work) = journal_over(instances);
        let usecase = CompleteCeremonyStepUseCase::new(
            definition_resolver(definitions),
            journal,
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

        let facts = unit_of_work.facts().await;
        assert_eq!(facts.len(), 1, "one ending, one fact: {facts:?}");
        assert_eq!(facts[0].event_type, AuditEventType::StepCompleted);
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
        let instances = Arc::new(InstanceRepositoryFake::default());
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
        let (journal, unit_of_work) = journal_over(instances);
        let usecase = CompleteCeremonyStepUseCase::new(
            definition_resolver(definitions),
            journal,
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
        let facts = unit_of_work.facts().await;
        assert_eq!(facts.len(), 1);
        assert!(facts[0].event_id.as_str().contains("iteration:1"));
    }
}
