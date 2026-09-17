//! [`RunCeremonyStepUseCase`] — acquire a step lease and invoke a handler.

use std::sync::Arc;

use made_core::entities::ceremony_commands::{ApplyStepResult, StartStep};
use made_core::entities::CeremonyCommand;
use made_core::error::DomainError;
use made_core::ports::{CeremonyStepHandlerPort, CeremonyStepHandlerRequest, ClockPort};
use made_core::value_objects::{StepErrorMessage, StepLease, StepResult};

use super::resolve_ceremony_definition_use_case::ResolveCeremonyDefinitionUseCase;
use super::run_ceremony_step_input::RunCeremonyStepInput;
use super::run_ceremony_step_output::RunCeremonyStepOutput;
use crate::services::{
    ceremony_transcript_projection, session_facts, ConflictPolicy, SessionStream,
};

pub struct RunCeremonyStepUseCase {
    definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    stream: Arc<SessionStream>,
    handler: Arc<dyn CeremonyStepHandlerPort>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for RunCeremonyStepUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunCeremonyStepUseCase").finish()
    }
}

impl RunCeremonyStepUseCase {
    #[must_use]
    pub fn new(
        definitions: Arc<ResolveCeremonyDefinitionUseCase>,
        stream: Arc<SessionStream>,
        handler: Arc<dyn CeremonyStepHandlerPort>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            definitions,
            stream,
            handler,
            clock,
        }
    }

    #[tracing::instrument(
        name = "run_ceremony_step",
        skip_all,
        fields(ceremony_id = %input.instance_id, step_id = %input.step_id)
    )]
    pub async fn execute(
        &self,
        input: RunCeremonyStepInput,
    ) -> Result<RunCeremonyStepOutput, DomainError> {
        // Two appends, not one, and deliberately so: the claim has to
        // be durable before the handler is invoked, or a crash while it
        // runs leaves no record that anything took the step.
        let session = self.stream.load(&input.instance_id).await?;
        // Resolved from the instance, never from the request: a session
        // bound to a published version must be advanced by the very
        // definition it recorded, and one that is unbound has only the
        // repository to go to. Reading coordinates off the caller made
        // a bound session unadvanceable, because publishing writes to
        // the catalogue and not to the repository.
        let definition = self.definitions.execute(&session.instance).await?;
        let step = definition
            .step(&input.step_id)
            .cloned()
            .ok_or(DomainError::NotFound {
                what: "ceremony_step",
            })?;
        let actor = session_facts::seat(&input.role_id, input.role_kind)?;

        let now = self.clock.now();
        let lease = StepLease::acquire(
            input.lease_owner_id,
            input.idempotency_key,
            now,
            input.lease_ttl,
        )?;
        let claim = CeremonyCommand::StartStep(StartStep {
            role_id: Some(input.role_id.clone()),
            step_id: input.step_id.clone(),
            lease,
            now,
        });
        let claimed = self
            .stream
            .execute(session, ConflictPolicy::retry(), |session| {
                let events = session.instance.decide(&claim, &definition)?;
                session_facts::facts(&session.instance, events, &actor, now)
            })
            .await?;
        let instance = claimed.instance;
        // Captured off the claim, before the result is applied: a
        // successful repeat advances the record to the next iteration.
        let record = instance
            .step_record(&input.step_id)
            .ok_or(DomainError::NotFound {
                what: "ceremony_step",
            })?;
        let attempt = record.attempt();

        // What was said so far, folded from the stream: every step
        // that completed is in it, however it was driven.
        let transcript =
            ceremony_transcript_projection::transcript(&self.stream.records(instance.id()).await?);
        let request = CeremonyStepHandlerRequest::new(
            instance.id().clone(),
            instance.definition_name().clone(),
            instance.definition_version().clone(),
            instance.current_state().clone(),
            step.id().clone(),
            step.handler_kind().clone(),
            step.handler_config().clone(),
            instance.context().clone(),
            attempt,
        )
        .with_transcript(transcript)
        .with_interventions(instance.interventions().to_vec())
        .with_role(input.role_id.clone())
        .with_bound_specialty(instance.bound_specialty(&input.role_id).cloned());
        let result = self.execute_handler(request).await?;

        // Loaded again rather than reusing what the claim left: the
        // handler may have taken a while, and the version that was
        // current then is not the one this append has to expect.
        let session = self.stream.load(instance.id()).await?;
        let finished_at = self.clock.now();
        let finish = CeremonyCommand::ApplyStepResult(ApplyStepResult {
            step_id: input.step_id.clone(),
            result: result.clone(),
            now: finished_at,
        });
        let refreshed = self
            .stream
            .execute(session, ConflictPolicy::retry(), |session| {
                let events = session.instance.decide(&finish, &definition)?;
                session_facts::facts(&session.instance, events, &actor, finished_at)
            })
            .await?
            .instance;
        Ok(RunCeremonyStepOutput::new(refreshed, attempt, result))
    }

    async fn execute_handler(
        &self,
        request: CeremonyStepHandlerRequest,
    ) -> Result<StepResult, DomainError> {
        match self.handler.execute(request).await {
            Ok(result) => Ok(result),
            Err(error) => {
                let message = StepErrorMessage::new(error.to_string())?;
                StepResult::failed(message)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use made_core::error::DomainError;
    use made_core::value_objects::{
        Attributes, AuditActorKind, AuditEventType, StepAttempt, StepErrorMessage, StepOutput,
        StepStatus,
    };

    use super::*;
    use crate::usecases::ceremony_test_support::{
        approval_definition, ceremony_id, definition, definition_resolver, idempotency_key,
        lease_owner, lease_ttl, now, repeating_approval_definition, resolver_with, role_id,
        started_instance, step_id, stream, stream_over, DefinitionRepositoryFake, EventStoreFake,
        FixedClock, PublicationsFake, SequenceStepHandlerFake, StepHandlerFake,
    };
    use crate::usecases::GetCeremonyTranscriptUseCase;

    fn readiness_output(ready: bool) -> StepOutput {
        StepOutput::new(
            Attributes::new(BTreeMap::from([(
                "ready".to_owned(),
                serde_json::json!(ready),
            )]))
            .unwrap(),
        )
    }

    /// The regression this whole change exists for. Publishing writes
    /// to the catalogue and nowhere else, so a session bound to a
    /// published version used to be startable and then unadvanceable:
    /// the step resolved its definition from the repository, which had
    /// never heard of it. Here the repository deliberately holds a
    /// different ceremony, so the step can only run if resolution
    /// followed the binding.
    #[tokio::test]
    async fn a_bound_session_runs_the_definition_it_was_published_from() {
        let publications = Arc::new(PublicationsFake::default());
        let published = publications.seed(definition()).await;
        let elsewhere = Arc::new(DefinitionRepositoryFake::new(approval_definition()));
        let instances = Arc::new(EventStoreFake::default());
        instances
            .save(&made_core::entities::CeremonyInstance::start_bound(
                ceremony_id(),
                &published,
                made_core::value_objects::CeremonyContext::empty(),
                now(),
            ))
            .await
            .unwrap();
        let usecase = RunCeremonyStepUseCase::new(
            resolver_with(elsewhere, publications),
            stream(instances.clone()),
            Arc::new(StepHandlerFake::succeeding(
                StepResult::completed(StepOutput::empty()).unwrap(),
            )),
            Arc::new(FixedClock::new(now())),
        );

        let output = usecase
            .execute(RunCeremonyStepInput::new(
                ceremony_id(),
                role_id(),
                AuditActorKind::Agent,
                step_id(),
                lease_owner(),
                idempotency_key("bound-1"),
                lease_ttl(),
            ))
            .await
            .expect("a bound session must be advanceable by what it is bound to");

        assert_eq!(output.result().status(), StepStatus::Completed);
        assert_eq!(
            output.instance().bound_definition(),
            Some(published.digest()),
            "advancing must not quietly unbind the session"
        );
    }

    #[tokio::test]
    async fn invokes_handler_and_persists_completed_result() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        instances
            .save(&started_instance(&definition))
            .await
            .unwrap();
        let handler = Arc::new(StepHandlerFake::succeeding(
            StepResult::completed(StepOutput::empty()).unwrap(),
        ));
        let usecase = RunCeremonyStepUseCase::new(
            definition_resolver(definitions),
            stream(instances.clone()),
            handler.clone(),
            Arc::new(FixedClock::new(now())),
        );

        let output = usecase
            .execute(RunCeremonyStepInput::new(
                ceremony_id(),
                role_id(),
                AuditActorKind::Agent,
                step_id(),
                lease_owner(),
                idempotency_key("run-1"),
                lease_ttl(),
            ))
            .await
            .unwrap();

        assert_eq!(output.attempt(), StepAttempt::FIRST);
        assert_eq!(output.result().status(), StepStatus::Completed);
        let requests = handler.requests().await;
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].step_id(), &step_id());
        assert_eq!(requests[0].handler_kind().as_str(), "multiagent_round");
        assert_eq!(requests[0].role_id(), Some(&role_id()));
        assert!(requests[0].transcript().is_empty());
        assert!(requests[0].interventions().is_empty());
        let transcript = GetCeremonyTranscriptUseCase::new(instances.clone())
            .execute(&ceremony_id())
            .await
            .unwrap();
        assert_eq!(transcript.len(), 1);
        assert_eq!(transcript.contributions()[0].step_id(), &step_id());
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
    async fn incremental_execution_exposes_same_step_as_next_iteration() {
        let definition = repeating_approval_definition(3);
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        instances
            .save(&started_instance(&definition))
            .await
            .unwrap();
        let handler = Arc::new(SequenceStepHandlerFake::new([
            StepResult::completed(readiness_output(false)).unwrap(),
            StepResult::completed(readiness_output(true)).unwrap(),
        ]));
        let usecase = RunCeremonyStepUseCase::new(
            definition_resolver(definitions),
            stream(instances),
            handler,
            Arc::new(FixedClock::new(now())),
        );

        let first = usecase
            .execute(RunCeremonyStepInput::new(
                ceremony_id(),
                role_id(),
                AuditActorKind::Agent,
                step_id(),
                lease_owner(),
                idempotency_key("repeat-run-1"),
                lease_ttl(),
            ))
            .await
            .unwrap();
        let first_view =
            crate::usecases::CeremonyInstanceView::project(first.instance(), &definition).unwrap();
        assert_eq!(first_view.next_step_id(), Some(&step_id()));
        assert!(first_view.waiting_for_human().is_empty());
        assert_eq!(
            first
                .instance()
                .step_record(&step_id())
                .unwrap()
                .iteration()
                .get(),
            2
        );

        let second = usecase
            .execute(RunCeremonyStepInput::new(
                ceremony_id(),
                role_id(),
                AuditActorKind::Agent,
                step_id(),
                lease_owner(),
                idempotency_key("repeat-run-2"),
                lease_ttl(),
            ))
            .await
            .unwrap();
        let second_view =
            crate::usecases::CeremonyInstanceView::project(second.instance(), &definition).unwrap();
        assert_eq!(second_view.next_step_id(), None);
        assert!(second_view.steps()[0].repeat_condition_satisfied());
        assert!(!second_view.steps()[0].repeat_limit_reached());
        assert_eq!(second_view.waiting_for_human().len(), 1);
    }

    #[tokio::test]
    async fn handler_domain_error_is_persisted_as_failed_step() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        instances
            .save(&started_instance(&definition))
            .await
            .unwrap();
        let handler = Arc::new(StepHandlerFake::failing(DomainError::InvariantViolated {
            reason: "handler rejected step",
        }));
        let usecase = RunCeremonyStepUseCase::new(
            definition_resolver(definitions),
            stream(instances.clone()),
            handler,
            Arc::new(FixedClock::new(now())),
        );

        let output = usecase
            .execute(RunCeremonyStepInput::new(
                ceremony_id(),
                role_id(),
                AuditActorKind::Agent,
                step_id(),
                lease_owner(),
                idempotency_key("run-1"),
                lease_ttl(),
            ))
            .await
            .unwrap();

        assert_eq!(output.result().status(), StepStatus::Failed);
        let saved = instances.saved(&ceremony_id()).await;
        let record = saved.step_record(&step_id()).unwrap();
        assert_eq!(record.status(), StepStatus::Failed);
        assert!(record.error_message().is_some());
    }

    #[tokio::test]
    async fn active_lease_blocks_runner_before_handler_invocation() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        let mut instance = started_instance(&definition);
        instance
            .start_step(
                &definition,
                &step_id(),
                StepLease::new(
                    lease_owner(),
                    idempotency_key("existing-run"),
                    now(),
                    now() + time::Duration::seconds(60),
                )
                .unwrap(),
                now(),
            )
            .unwrap();
        instances.save(&instance).await.unwrap();
        let handler = Arc::new(StepHandlerFake::succeeding(
            StepResult::completed(StepOutput::empty()).unwrap(),
        ));
        let usecase = RunCeremonyStepUseCase::new(
            definition_resolver(definitions),
            stream(instances),
            handler.clone(),
            Arc::new(FixedClock::new(now())),
        );

        let err = usecase
            .execute(RunCeremonyStepInput::new(
                ceremony_id(),
                role_id(),
                AuditActorKind::Agent,
                step_id(),
                lease_owner(),
                idempotency_key("new-run"),
                lease_ttl(),
            ))
            .await
            .unwrap_err();

        assert!(matches!(err, DomainError::InvariantViolated { .. }));
        assert!(handler.requests().await.is_empty());
    }

    /// Taking a step and finishing it are two facts, and they are
    /// committed separately on purpose.
    ///
    /// The claim has to be durable before the handler is invoked. A
    /// crash while the handler runs must leave a session that says
    /// somebody took this step and never came back — not one that looks
    /// untouched.
    #[tokio::test]
    async fn seals_the_claim_before_the_work_and_the_ending_after() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        instances
            .save(&started_instance(&definition))
            .await
            .unwrap();
        let (stream, store) = stream_over(instances);
        let usecase = RunCeremonyStepUseCase::new(
            definition_resolver(definitions),
            stream,
            Arc::new(StepHandlerFake::succeeding(
                StepResult::completed(StepOutput::empty()).unwrap(),
            )),
            Arc::new(FixedClock::new(now())),
        );

        usecase
            .execute(RunCeremonyStepInput::new(
                ceremony_id(),
                role_id(),
                AuditActorKind::Agent,
                step_id(),
                lease_owner(),
                idempotency_key("run-1"),
                lease_ttl(),
            ))
            .await
            .unwrap();

        let facts = store.facts().await;
        let sealed = facts
            .iter()
            .map(|fact| fact.event.event_type())
            .collect::<Vec<_>>();
        assert_eq!(
            sealed,
            vec![AuditEventType::StepStarted, AuditEventType::StepCompleted],
            "taking the step and finishing it are two facts: {facts:?}"
        );
        assert!(facts
            .iter()
            .all(|fact| fact.actor.kind() == AuditActorKind::Agent));
    }

    /// A step that fails says so, rather than saying it ended.
    ///
    /// "Did anything fail here" is the first question asked of a
    /// session that went wrong, and it should not need reading into
    /// every entry to answer.
    #[tokio::test]
    async fn a_failed_step_is_sealed_as_a_failure() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        instances
            .save(&started_instance(&definition))
            .await
            .unwrap();
        let (stream, store) = stream_over(instances);
        let usecase = RunCeremonyStepUseCase::new(
            definition_resolver(definitions),
            stream,
            Arc::new(StepHandlerFake::succeeding(
                StepResult::failed(StepErrorMessage::new("the handler gave up").unwrap()).unwrap(),
            )),
            Arc::new(FixedClock::new(now())),
        );

        usecase
            .execute(RunCeremonyStepInput::new(
                ceremony_id(),
                role_id(),
                AuditActorKind::Agent,
                step_id(),
                lease_owner(),
                idempotency_key("run-1"),
                lease_ttl(),
            ))
            .await
            .unwrap();

        let sealed = store
            .facts()
            .await
            .iter()
            .map(|fact| fact.event.event_type())
            .collect::<Vec<_>>();
        assert_eq!(
            sealed,
            vec![AuditEventType::StepStarted, AuditEventType::StepFailed],
            "a step that failed was sealed as something else"
        );
    }
}
