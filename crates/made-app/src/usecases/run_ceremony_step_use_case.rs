//! [`RunCeremonyStepUseCase`] — acquire a step lease and invoke a handler.

use std::sync::Arc;

use made_core::entities::ceremony_commands::ApplyStepResult;
use made_core::entities::{CeremonyCommand, CeremonyDefinition};
use made_core::error::DomainError;
use made_core::ports::{CeremonyStepHandlerPort, CeremonyStepHandlerRequest, ClockPort};
use made_core::value_objects::{MaxParallel, RoleId, StepExecutionRecord, StepId, StepResult};

mod claim;
mod spawn;

use super::resolve_ceremony_definition_use_case::ResolveCeremonyDefinitionUseCase;
use super::run_ceremony_step_input::RunCeremonyStepInput;
use super::run_ceremony_step_output::RunCeremonyStepOutput;
use super::{prepare_step_execution, PrepareCeremonyChildrenUseCase, PreparedStepExecution};
use crate::services::{
    ceremony_transcript_projection, session_facts, ConflictPolicy, SessionStream,
};

pub struct RunCeremonyStepUseCase {
    definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    stream: Arc<SessionStream>,
    handler: Arc<dyn CeremonyStepHandlerPort>,
    clock: Arc<dyn ClockPort>,
    max_parallel_ceiling: MaxParallel,
    children: Option<Arc<PrepareCeremonyChildrenUseCase>>,
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
            max_parallel_ceiling: MaxParallel::SERVER_MAX,
            children: None,
        }
    }

    #[must_use]
    pub fn with_max_parallel_ceiling(mut self, ceiling: MaxParallel) -> Self {
        self.max_parallel_ceiling = ceiling;
        self
    }

    #[must_use]
    pub fn with_child_orchestrator(
        mut self,
        children: Arc<PrepareCeremonyChildrenUseCase>,
    ) -> Self {
        self.children = Some(children);
        self
    }

    #[tracing::instrument(
        name = "run_ceremony_step",
        skip_all,
        fields(ceremony_id = %input.instance_id, step_id = %input.step_id,
               state_visit = tracing::field::Empty,
               state_iteration = tracing::field::Empty, iteration = tracing::field::Empty,
               attempt = tracing::field::Empty)
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
        if step.spawn().is_some() && self.children.is_none() {
            return Err(DomainError::InvariantViolated {
                reason: "child-spawning step requires the child orchestrator",
            });
        }
        let requested_role_id = input.requested_role_id();
        let actor_kind = input.role_kind;

        let claimed = self
            .claim_step(session, &definition, &input, requested_role_id, actor_kind)
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
        let claim_fence = instance.step_claim_fence(&input.step_id)?;
        let sealed_role = sealed_role(record, &definition, &input.step_id)?;
        super::step_span::record_coordinates(
            record.state_visit(),
            record.state_iteration(),
            record.iteration(),
            attempt,
        );

        if step.spawn().is_some() {
            return self
                .execute_spawn_step(
                    instance,
                    input.step_id,
                    claim_fence,
                    input.role_kind,
                    attempt,
                )
                .await;
        }

        // What was said so far, folded from the stream: every step
        // that completed is in it, however it was driven.
        let transcript =
            ceremony_transcript_projection::transcript(&self.stream.records(instance.id()).await?);
        let result =
            match prepare_step_execution(&definition, &step, record.state_visit(), transcript) {
                Ok(PreparedStepExecution::Handler { transcript }) => {
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
                    .with_role(sealed_role.clone())
                    .with_bound_specialty(instance.bound_specialty(&sealed_role).cloned());
                    self.execute_handler(request).await?
                }
                Ok(PreparedStepExecution::Deterministic { result }) => result,
                Err(error) => {
                    super::step_span::record_error(&error);
                    StepResult::from_handler_error(&error)?
                }
            };

        // Loaded again rather than reusing what the claim left: the
        // handler may have taken a while, and the version that was
        // current then is not the one this append has to expect.
        let session = self.stream.load(instance.id()).await?;
        let finished_at = self.clock.now();
        let finish = CeremonyCommand::ApplyStepResult(ApplyStepResult {
            step_id: input.step_id.clone(),
            result: result.clone(),
            claim_fence,
            now: finished_at,
        });
        let finish_actor_kind = input.role_kind;
        let refreshed = self
            .stream
            .execute(session, ConflictPolicy::retry(), |session| {
                let events = session.instance.decide(&finish, &definition)?;
                if events.is_empty() {
                    return Ok(Vec::new());
                }
                let finish_actor = session_facts::step_result_seat(&events, finish_actor_kind)?;
                session_facts::facts(&session.instance, events, &finish_actor, finished_at)
            })
            .await?
            .instance;
        Ok(RunCeremonyStepOutput::new(refreshed, attempt, result))
    }

    #[tracing::instrument(
        name = "ceremony_step_handler",
        skip_all,
        fields(
            ceremony_id = %request.instance_id(),
            step_id = %request.step_id(),
            handler_kind = %request.handler_kind(),
            attempt = tracing::field::Empty,
            outcome = tracing::field::Empty,
            step_status = tracing::field::Empty,
            error_kind = tracing::field::Empty,
        )
    )]
    async fn execute_handler(
        &self,
        request: CeremonyStepHandlerRequest,
    ) -> Result<StepResult, DomainError> {
        super::step_span::record_attempt(request.attempt());
        match self.handler.execute(request).await {
            Ok(result) => {
                super::step_span::record_result(&result);
                Ok(result)
            }
            Err(error) => {
                super::step_span::record_error(&error);
                let result = StepResult::from_handler_error(&error)?;
                super::step_span::record_status(&result);
                Ok(result)
            }
        }
    }
}

fn sealed_role(
    record: &StepExecutionRecord,
    definition: &CeremonyDefinition,
    step_id: &StepId,
) -> Result<RoleId, DomainError> {
    record
        .claimed_role()
        .cloned()
        .map_or_else(|| definition.role_id_for_step(step_id), Ok)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use made_core::error::DomainError;
    use made_core::value_objects::{
        Attributes, AuditActorKind, AuditEventType, StepAttempt, StepErrorMessage, StepLease,
        StepOutput, StepStatus,
    };

    use super::*;
    use crate::usecases::ceremony_test_support::{
        a_memory, approval_definition, ceremony_id, child_spawning_definition, definition,
        definition_resolver, idempotency_key, lease_owner, lease_ttl, now,
        repeating_approval_definition, resolver_with, review_child_definition, role_id,
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

    #[tokio::test]
    async fn a_spawn_step_without_an_orchestrator_is_refused_before_the_claim() {
        let definition = child_spawning_definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let store = Arc::new(EventStoreFake::default());
        store.save(&started_instance(&definition)).await.unwrap();
        let handler = Arc::new(StepHandlerFake::succeeding(
            StepResult::completed(StepOutput::empty()).unwrap(),
        ));
        let usecase = RunCeremonyStepUseCase::new(
            definition_resolver(definitions),
            stream(store.clone()),
            handler.clone(),
            Arc::new(FixedClock::new(now())),
        );

        let error = usecase
            .execute(RunCeremonyStepInput::new(
                ceremony_id(),
                role_id(),
                AuditActorKind::Agent,
                step_id(),
                lease_owner(),
                idempotency_key("spawn-without-orchestrator"),
                lease_ttl(),
            ))
            .await
            .unwrap_err();

        assert!(error.to_string().contains("child orchestrator"));
        assert!(handler.requests().await.is_empty());
        assert!(
            store.facts().await.is_empty(),
            "a refused spawn was claimed"
        );
    }

    #[tokio::test]
    async fn a_spawn_step_opens_every_child_through_the_configured_orchestrator() {
        let definition = child_spawning_definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let publications = Arc::new(PublicationsFake::default());
        publications.seed(review_child_definition()).await;
        let store = Arc::new(EventStoreFake::default());
        store.save(&started_instance(&definition)).await.unwrap();
        let stream = stream(store.clone());
        let resolver = resolver_with(definitions, publications.clone());
        let children = Arc::new(PrepareCeremonyChildrenUseCase::new(
            resolver.clone(),
            publications,
            stream.clone(),
            Arc::new(FixedClock::new(now())),
            a_memory(),
        ));
        let handler = Arc::new(StepHandlerFake::failing(DomainError::InvariantViolated {
            reason: "spawn steps must not reach a handler",
        }));
        let usecase = RunCeremonyStepUseCase::new(
            resolver,
            stream,
            handler.clone(),
            Arc::new(FixedClock::new(now())),
        )
        .with_child_orchestrator(children);

        let output = usecase
            .execute(RunCeremonyStepInput::new(
                ceremony_id(),
                role_id(),
                AuditActorKind::Agent,
                step_id(),
                lease_owner(),
                idempotency_key("spawn-with-orchestrator"),
                lease_ttl(),
            ))
            .await
            .unwrap();

        assert_eq!(output.result().status(), StepStatus::Completed);
        assert!(handler.requests().await.is_empty());
        let group = output.instance().child_groups().values().next().unwrap();
        assert_eq!(group.plan().children().len(), 1);
        let child = group.plan().children()[0].child_id();
        assert!(
            store.exists(child).await,
            "the planned child was not opened"
        );
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
            .save(
                &made_core::entities::CeremonyInstance::start_bound(
                    ceremony_id(),
                    &published,
                    made_core::value_objects::CeremonyContext::empty(),
                    now(),
                )
                .expect("required ceremony inputs"),
            )
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
    async fn no_valid_proposal_is_sealed_with_its_typed_classification() {
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
            Arc::new(StepHandlerFake::failing(DomainError::NoValidProposal {
                contract_id: "review".to_owned(),
            })),
            Arc::new(FixedClock::new(now())),
        );
        let output = usecase
            .execute(RunCeremonyStepInput::new(
                ceremony_id(),
                role_id(),
                AuditActorKind::Agent,
                step_id(),
                lease_owner(),
                idempotency_key("classified-failure"),
                lease_ttl(),
            ))
            .await
            .unwrap();
        assert_eq!(
            output.result().failure_kind(),
            Some(made_core::value_objects::StepFailureKind::NoValidProposal)
        );
        let facts = store.facts().await;
        let made_core::entities::CeremonyEvent::StepFailed(failed) = &facts.last().unwrap().event
        else {
            panic!("expected failure")
        };
        assert_eq!(failed.result.failure_kind(), output.result().failure_kind());
        assert_eq!(
            facts.last().unwrap().event.schema_version(),
            made_core::value_objects::EventSchemaVersion::V4
        );
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
