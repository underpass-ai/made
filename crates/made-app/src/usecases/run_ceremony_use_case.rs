//! [`RunCeremonyUseCase`] — execute a declarative ceremony to terminal state.

use std::sync::Arc;

use crate::services::{
    ceremony_transcript_projection, session_facts, ConflictPolicy, SessionStream,
};
use made_core::entities::ceremony_commands::ApplyTransition;
use made_core::entities::{CeremonyCommand, CeremonyInstance};
use made_core::error::DomainError;
use made_core::ports::{
    CeremonyDefinitionRepositoryPort, CeremonyStepHandlerPort, ClockPort, MetricsRecorderPort,
    NoopMetricsRecorder,
};
use made_core::value_objects::CeremonyOutcome;

use super::ceremony_step_trace::CeremonyStepTrace;
use super::run_ceremony_input::RunCeremonyInput;
use super::run_ceremony_output::RunCeremonyOutput;

mod steps;

/// Drives a declarative ceremony through its steps and transitions.
pub struct RunCeremonyUseCase {
    definitions: Arc<dyn CeremonyDefinitionRepositoryPort>,
    stream: Arc<SessionStream>,
    handler: Arc<dyn CeremonyStepHandlerPort>,
    clock: Arc<dyn ClockPort>,
    metrics: Arc<dyn MetricsRecorderPort>,
}

impl std::fmt::Debug for RunCeremonyUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunCeremonyUseCase").finish()
    }
}

impl RunCeremonyUseCase {
    #[must_use]
    pub fn new(
        definitions: Arc<dyn CeremonyDefinitionRepositoryPort>,
        stream: Arc<SessionStream>,
        handler: Arc<dyn CeremonyStepHandlerPort>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            definitions,
            stream,
            handler,
            clock,
            metrics: Arc::new(NoopMetricsRecorder),
        }
    }

    /// Count driver refusals and blocked transitions that produce no event.
    /// Event subscribers record committed outcomes, durations and step status.
    #[must_use]
    pub fn with_metrics(mut self, metrics: Arc<dyn MetricsRecorderPort>) -> Self {
        self.metrics = metrics;
        self
    }

    // The ceremony driver is one cohesive FSM loop; splitting it would
    // scatter the state-machine logic and its interleaved instrumentation.
    #[allow(clippy::too_many_lines)]
    #[tracing::instrument(
        name = "run_ceremony",
        skip_all,
        fields(ceremony_id = %input.id())
    )]
    pub async fn execute(&self, input: RunCeremonyInput) -> Result<RunCeremonyOutput, DomainError> {
        let (id, definition, context, lease_owner_id, lease_ttl, actor_id, actor_kind) =
            input.into_parts();
        let ceremony_name = definition.name().as_str().to_owned();
        // Asked before the definition is stored, so a run that is
        // about to be refused does not leave one behind. This is a
        // courtesy and not the guard: two runs can still both get past
        // it, and what stops the second is the append below expecting
        // the stream to be empty.
        if !matches!(
            self.stream.load(&id).await,
            Err(DomainError::NotFound { .. })
        ) {
            self.metrics
                .record_ceremony_outcome(&ceremony_name, CeremonyOutcome::AlreadyExists);
            return Err(DomainError::AlreadyExists {
                what: "ceremony_instance",
            });
        }
        let started_at = self.clock.now();
        let opener = session_facts::party(actor_id.as_str(), actor_kind)?;
        // The one-shot driver reads no memory. It takes a definition
        // handed to it and runs it end to end; a recollection is what a
        // session that outlives one call is opened with, and E1 gives
        // it to the two use cases that open one.
        let started =
            CeremonyInstance::decide_start(id.clone(), &definition, context, None, started_at)?;
        self.definitions.save(&definition).await?;
        // The guard proper: the append expects the stream to be empty,
        // so of two runs that both got past the check above, the loser
        // is told rather than winning quietly.
        let mut session = match self.stream.open(started, opener, started_at).await {
            Ok(session) => session,
            Err(error @ DomainError::AlreadyExists { .. }) => {
                self.metrics
                    .record_ceremony_outcome(&ceremony_name, CeremonyOutcome::AlreadyExists);
                return Err(error);
            }
            Err(error) => return Err(error),
        };

        let max_iterations = definition
            .states()
            .values()
            .map(|state| {
                state
                    .repeat_policy()
                    .map_or(1, |repeat| repeat.max_iterations().get() as usize)
            })
            .sum::<usize>()
            .saturating_add(definition.transitions().len())
            .saturating_add(1);
        let mut step_traces = Vec::new();
        'driver: for _ in 0..max_iterations {
            if session.instance.is_completed(&definition) {
                return Ok(RunCeremonyOutput::new(
                    definition,
                    session.instance,
                    step_traces,
                ));
            }

            let state_id = session.instance.current_state().clone();
            let state_iteration = session.instance.current_state_iteration();
            let step_ids = definition
                .steps_for_state(&state_id)
                .map(|step| step.id().clone())
                .collect::<Vec<_>>();
            for step_id in step_ids {
                loop {
                    if session
                        .instance
                        .step_record(&step_id)
                        .is_some_and(|record| record.status().is_success())
                    {
                        break;
                    }
                    let role_id = definition.role_id_for_step(&step_id)?;
                    let actor = session_facts::seat(&role_id, actor_kind)?;
                    // What was said so far, folded from the stream the
                    // steps before this one sealed.
                    let transcript = ceremony_transcript_projection::transcript(
                        &self.stream.records(&id).await?,
                    );
                    let (moved_on, executed_state_iteration, iteration, attempt, step_result) =
                        self.run_step(
                            &definition,
                            session,
                            &role_id,
                            &actor,
                            &step_id,
                            &lease_owner_id,
                            lease_ttl,
                            step_traces.len(),
                            transcript,
                        )
                        .await?;
                    session = moved_on;
                    step_traces.push(CeremonyStepTrace::for_coordinates(
                        state_id.clone(),
                        executed_state_iteration,
                        step_id.clone(),
                        role_id,
                        iteration,
                        attempt,
                        step_result.status(),
                        step_result.output().clone(),
                    ));
                    if !step_result.is_success() {
                        return Err(DomainError::InvariantViolated {
                            reason: "ceremony step did not complete successfully",
                        });
                    }
                    if session
                        .instance
                        .step_repeat_limit_reached(&definition, &step_id)
                    {
                        self.metrics
                            .record_ceremony_outcome(&ceremony_name, CeremonyOutcome::RepeatLimit);
                        return Err(DomainError::InvariantViolated {
                            reason: "ceremony step repeat limit exhausted",
                        });
                    }
                    if session.instance.current_state_iteration() != state_iteration {
                        continue 'driver;
                    }
                }
            }

            if session.instance.is_completed(&definition) {
                return Ok(RunCeremonyOutput::new(
                    definition,
                    session.instance,
                    step_traces,
                ));
            }
            if session.instance.state_repeat_limit_reached(&definition) {
                self.metrics
                    .record_ceremony_outcome(&ceremony_name, CeremonyOutcome::StateRepeatLimit);
                return Err(DomainError::InvariantViolated {
                    reason: "ceremony state repeat limit exhausted",
                });
            }
            let Some(transition) = definition
                .available_transitions(&state_id)
                .find(|transition| {
                    session
                        .instance
                        .transition_is_enabled(&definition, transition)
                })
            else {
                self.metrics
                    .record_ceremony_transition_blocked(&ceremony_name, state_id.as_str());
                self.metrics
                    .record_ceremony_outcome(&ceremony_name, CeremonyOutcome::NoTransition);
                return Err(DomainError::InvariantViolated {
                    reason: "no satisfied ceremony transition is available",
                });
            };
            let role_id = definition.role_id_for_transition(transition.trigger())?;
            let actor = session_facts::seat(&role_id, actor_kind)?;
            let moved_at = self.clock.now();
            let command = CeremonyCommand::ApplyTransition(ApplyTransition {
                role_id: Some(role_id),
                trigger: transition.trigger().clone(),
                now: moved_at,
            });
            // Fail fast, as the transition verb does: the move was
            // chosen against this state, and a stream that moved under
            // the driver is not one it should move again blind.
            session = self
                .stream
                .execute(session, ConflictPolicy::FailFast, |session| {
                    let events = session.instance.decide(&command, &definition)?;
                    session_facts::facts(&session.instance, events, &actor, moved_at)
                })
                .await?;
        }

        self.metrics
            .record_ceremony_outcome(&ceremony_name, CeremonyOutcome::IterationLimit);
        Err(DomainError::InvariantViolated {
            reason: "ceremony execution exceeded transition safety limit",
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use made_core::error::DomainError;
    use made_core::ports::CeremonyDefinitionRepositoryPort;
    use made_core::value_objects::{
        Attributes, AuditActorKind, AuditEventType, CeremonyContext, StepId, StepOutput,
        StepResult, StepStatus,
    };

    use super::*;
    use crate::usecases::ceremony_test_support::{
        approval_definition, ceremony_id, definition, lease_owner, lease_ttl, now,
        repeating_definition, started_instance, state_repeating_definition, step_id, stream,
        stream_over, two_step_definition, DefinitionRepositoryFake, EventStoreFake, FixedClock,
        SequenceStepHandlerFake, StepHandlerFake,
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
    async fn repeats_every_state_step_with_an_independent_durable_coordinate() {
        let definition = state_repeating_definition(3);
        let definitions = Arc::new(DefinitionRepositoryFake::default());
        let instances = Arc::new(EventStoreFake::default());
        let (stream, store) = stream_over(instances.clone());
        let handler = Arc::new(SequenceStepHandlerFake::new([
            StepResult::completed(StepOutput::empty()).unwrap(),
            StepResult::completed(readiness_output(false)).unwrap(),
            StepResult::completed(StepOutput::empty()).unwrap(),
            StepResult::completed(readiness_output(true)).unwrap(),
        ]));
        let usecase = RunCeremonyUseCase::new(
            definitions,
            stream,
            handler,
            Arc::new(FixedClock::new(now())),
        );

        let output = usecase
            .execute(RunCeremonyInput::new(
                ceremony_id(),
                definition.clone(),
                CeremonyContext::empty(),
                lease_owner(),
                lease_ttl(),
                "operator-1",
                AuditActorKind::Service,
            ))
            .await
            .unwrap();

        assert!(output.instance().is_completed(&definition));
        assert_eq!(
            output
                .step_traces()
                .iter()
                .map(|trace| trace.state_iteration().get())
                .collect::<Vec<_>>(),
            vec![1, 1, 2, 2]
        );
        assert!(output
            .step_traces()
            .iter()
            .all(|trace| trace.iteration().get() == 1 && trace.attempt().get() == 1));
        assert_eq!(
            output
                .instance()
                .step_record_history(&StepId::new("open").unwrap())
                .len(),
            1
        );
        assert_eq!(
            output.instance().transitions()[0].state_iteration().get(),
            2
        );
        let facts = store.facts().await;
        assert_eq!(
            facts
                .iter()
                .filter(|fact| fact.event.event_type() == AuditEventType::StateIterationStarted)
                .count(),
            1
        );
        let ids = facts
            .iter()
            .map(|fact| fact.event_id.as_str())
            .filter(|id| id.contains("step:"))
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(ids.len(), 8);
    }

    #[tokio::test]
    async fn state_repeat_exhaustion_is_stable_and_has_its_own_outcome() {
        let definition = state_repeating_definition(2);
        let definitions = Arc::new(DefinitionRepositoryFake::default());
        let instances = Arc::new(EventStoreFake::default());
        let handler = Arc::new(SequenceStepHandlerFake::new([
            StepResult::completed(StepOutput::empty()).unwrap(),
            StepResult::completed(readiness_output(false)).unwrap(),
            StepResult::completed(StepOutput::empty()).unwrap(),
            StepResult::completed(readiness_output(false)).unwrap(),
        ]));
        let usecase = RunCeremonyUseCase::new(
            definitions,
            stream(instances.clone()),
            handler,
            Arc::new(FixedClock::new(now())),
        );

        let error = usecase
            .execute(RunCeremonyInput::new(
                ceremony_id(),
                definition.clone(),
                CeremonyContext::empty(),
                lease_owner(),
                lease_ttl(),
                "operator-1",
                AuditActorKind::Service,
            ))
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            DomainError::InvariantViolated {
                reason: "ceremony state repeat limit exhausted"
            }
        ));
        let saved = instances.saved(&ceremony_id()).await;
        assert!(saved.state_repeat_limit_reached(&definition));
        assert!(!saved.state_repeat_permits_transition(&definition));
        assert_eq!(saved.current_state_iteration().get(), 2);
    }

    #[tokio::test]
    async fn executes_linear_ceremony_to_terminal_state() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::default());
        let instances = Arc::new(EventStoreFake::default());
        let handler = Arc::new(StepHandlerFake::succeeding(
            StepResult::completed(StepOutput::empty()).unwrap(),
        ));
        let usecase = RunCeremonyUseCase::new(
            definitions,
            stream(instances.clone()),
            handler.clone(),
            Arc::new(FixedClock::new(now())),
        );

        let output = usecase
            .execute(RunCeremonyInput::new(
                ceremony_id(),
                definition.clone(),
                CeremonyContext::empty(),
                lease_owner(),
                lease_ttl(),
                "operator-1",
                AuditActorKind::Service,
            ))
            .await
            .unwrap();

        assert!(output.instance().is_completed(&definition));
        assert_eq!(output.step_traces().len(), 1);
        assert_eq!(output.step_traces()[0].step_id(), &step_id());
        assert_eq!(output.step_traces()[0].status(), StepStatus::Completed);
        assert_eq!(handler.requests().await.len(), 1);
        assert!(instances
            .saved(&ceremony_id())
            .await
            .is_completed(&definition));
    }

    #[tokio::test]
    async fn repeats_successful_step_until_structured_condition_is_true() {
        let definition = repeating_definition(3);
        let definitions = Arc::new(DefinitionRepositoryFake::default());
        let instances = Arc::new(EventStoreFake::default());
        let (stream, store) = stream_over(instances.clone());
        let handler = Arc::new(SequenceStepHandlerFake::new([
            StepResult::completed(readiness_output(false)).unwrap(),
            StepResult::completed(readiness_output(true)).unwrap(),
        ]));
        let usecase = RunCeremonyUseCase::new(
            definitions,
            stream,
            handler.clone(),
            Arc::new(FixedClock::new(now())),
        );

        let output = usecase
            .execute(RunCeremonyInput::new(
                ceremony_id(),
                definition.clone(),
                CeremonyContext::empty(),
                lease_owner(),
                lease_ttl(),
                "operator-1",
                AuditActorKind::Service,
            ))
            .await
            .unwrap();

        assert!(output.instance().is_completed(&definition));
        assert_eq!(output.step_traces().len(), 2);
        assert_eq!(output.step_traces()[0].iteration().get(), 1);
        assert_eq!(output.step_traces()[1].iteration().get(), 2);
        assert_eq!(output.instance().step_record_history(&step_id()).len(), 1);
        let requests = handler.requests().await;
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[1].transcript().len(), 1);
        let step_event_ids = store
            .facts()
            .await
            .into_iter()
            .filter(|fact| {
                matches!(
                    fact.event.event_type(),
                    AuditEventType::StepStarted | AuditEventType::StepCompleted
                )
            })
            .map(|fact| fact.event_id.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(step_event_ids.len(), 4);
        assert!(step_event_ids.iter().any(|id| id.contains("iteration:1")));
        assert!(step_event_ids.iter().any(|id| id.contains("iteration:2")));
        assert_eq!(
            step_event_ids
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            4,
            "each iteration start and completion needs its own audit identity"
        );
    }

    #[tokio::test]
    async fn reports_repeat_limit_instead_of_spinning_or_transitioning() {
        let definition = repeating_definition(2);
        let definitions = Arc::new(DefinitionRepositoryFake::default());
        let instances = Arc::new(EventStoreFake::default());
        let handler = Arc::new(StepHandlerFake::succeeding(
            StepResult::completed(readiness_output(false)).unwrap(),
        ));
        let usecase = RunCeremonyUseCase::new(
            definitions,
            stream(instances.clone()),
            handler.clone(),
            Arc::new(FixedClock::new(now())),
        );

        let error = usecase
            .execute(RunCeremonyInput::new(
                ceremony_id(),
                definition.clone(),
                CeremonyContext::empty(),
                lease_owner(),
                lease_ttl(),
                "operator-1",
                AuditActorKind::Service,
            ))
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            DomainError::InvariantViolated {
                reason: "ceremony step repeat limit exhausted"
            }
        ));
        assert_eq!(handler.requests().await.len(), 2);
        let saved = instances.saved(&ceremony_id()).await;
        assert!(!saved.is_completed(&definition));
        assert!(saved.step_repeat_limit_reached(&definition, &step_id()));
        assert_eq!(saved.step_record_history(&step_id()).len(), 1);
    }

    #[tokio::test]
    async fn aborts_when_a_step_does_not_complete_successfully() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::default());
        let instances = Arc::new(EventStoreFake::default());
        let handler = Arc::new(StepHandlerFake::failing(DomainError::InvariantViolated {
            reason: "handler rejected step",
        }));
        let usecase = RunCeremonyUseCase::new(
            definitions,
            stream(instances.clone()),
            handler,
            Arc::new(FixedClock::new(now())),
        );

        let err = usecase
            .execute(RunCeremonyInput::new(
                ceremony_id(),
                definition.clone(),
                CeremonyContext::empty(),
                lease_owner(),
                lease_ttl(),
                "operator-1",
                AuditActorKind::Service,
            ))
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            DomainError::InvariantViolated {
                reason: "ceremony step did not complete successfully"
            }
        ));
        // The failed step is recorded; the ceremony did not reach its
        // terminal state.
        assert!(!instances
            .saved(&ceremony_id())
            .await
            .is_completed(&definition));
    }

    #[tokio::test]
    async fn fails_when_no_outgoing_transition_is_satisfied() {
        // The approval ceremony can only advance through a human-approval
        // guard; with no approval in the context, no transition is
        // enabled out of the initial state.
        let definition = approval_definition();
        let definitions = Arc::new(DefinitionRepositoryFake::default());
        let instances = Arc::new(EventStoreFake::default());
        let handler = Arc::new(StepHandlerFake::succeeding(
            StepResult::completed(StepOutput::empty()).unwrap(),
        ));
        let usecase = RunCeremonyUseCase::new(
            definitions,
            stream(instances),
            handler.clone(),
            Arc::new(FixedClock::new(now())),
        );

        let err = usecase
            .execute(RunCeremonyInput::new(
                ceremony_id(),
                definition,
                CeremonyContext::empty(),
                lease_owner(),
                lease_ttl(),
                "operator-1",
                AuditActorKind::Service,
            ))
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            DomainError::InvariantViolated {
                reason: "no satisfied ceremony transition is available"
            }
        ));
        // The approval ceremony declares no steps, so the handler is
        // never invoked.
        assert!(handler.requests().await.is_empty());
    }

    #[tokio::test]
    async fn duplicate_instance_id_is_rejected() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::default());
        let instances = Arc::new(EventStoreFake::default());
        let handler = Arc::new(StepHandlerFake::succeeding(
            StepResult::completed(StepOutput::empty()).unwrap(),
        ));
        let usecase = RunCeremonyUseCase::new(
            definitions,
            stream(instances),
            handler,
            Arc::new(FixedClock::new(now())),
        );
        let input = || {
            RunCeremonyInput::new(
                ceremony_id(),
                definition.clone(),
                CeremonyContext::empty(),
                lease_owner(),
                lease_ttl(),
                "operator-1",
                AuditActorKind::Service,
            )
        };

        usecase.execute(input()).await.unwrap();
        let err = usecase.execute(input()).await.unwrap_err();

        assert!(matches!(
            err,
            DomainError::AlreadyExists {
                what: "ceremony_instance"
            }
        ));
    }

    #[tokio::test]
    async fn duplicate_instance_id_is_rejected_before_saving_definition() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::default());
        let instances = Arc::new(EventStoreFake::default());
        instances
            .save(&started_instance(&definition))
            .await
            .unwrap();
        let handler = Arc::new(StepHandlerFake::succeeding(
            StepResult::completed(StepOutput::empty()).unwrap(),
        ));
        let usecase = RunCeremonyUseCase::new(
            definitions.clone(),
            stream(instances),
            handler.clone(),
            Arc::new(FixedClock::new(now())),
        );

        let err = usecase
            .execute(RunCeremonyInput::new(
                ceremony_id(),
                definition,
                CeremonyContext::empty(),
                lease_owner(),
                lease_ttl(),
                "operator-1",
                AuditActorKind::Service,
            ))
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            DomainError::AlreadyExists {
                what: "ceremony_instance"
            }
        ));
        assert!(definitions.list().await.unwrap().is_empty());
        assert!(handler.requests().await.is_empty());
    }

    #[tokio::test]
    async fn threads_prior_step_output_into_the_next_step() {
        let definition = two_step_definition();
        let definitions = Arc::new(DefinitionRepositoryFake::default());
        let instances = Arc::new(EventStoreFake::default());
        let handler = Arc::new(StepHandlerFake::succeeding(
            StepResult::completed(StepOutput::empty()).unwrap(),
        ));
        let usecase = RunCeremonyUseCase::new(
            definitions,
            stream(instances),
            handler.clone(),
            Arc::new(FixedClock::new(now())),
        );

        usecase
            .execute(RunCeremonyInput::new(
                ceremony_id(),
                definition,
                CeremonyContext::empty(),
                lease_owner(),
                lease_ttl(),
                "operator-1",
                AuditActorKind::Service,
            ))
            .await
            .unwrap();

        let requests = handler.requests().await;
        assert_eq!(requests.len(), 2);
        // The first step opens the meeting with an empty transcript.
        assert!(requests[0].transcript().is_empty());
        // The second step receives the first step's contribution.
        let transcript = requests[1].transcript();
        assert_eq!(transcript.len(), 1);
        assert_eq!(transcript.contributions()[0].step_id().as_str(), "open");
        assert_eq!(
            transcript.contributions()[0].role_id().as_str(),
            "FACILITATOR"
        );
    }

    /// One run, and the journal reads back as the session's whole
    /// history.
    ///
    /// The driver carries its own copy of the lifecycle, so nothing
    /// forces it to seal what the standalone verbs seal. This is the
    /// assertion that it does: a caller who ran a ceremony end to end
    /// and one who drove it verb by verb leave the same record.
    #[tokio::test]
    async fn seals_the_whole_run_as_the_verbs_would_have() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        let (stream, store) = stream_over(instances);
        let usecase = RunCeremonyUseCase::new(
            definitions,
            stream,
            Arc::new(StepHandlerFake::succeeding(
                StepResult::completed(StepOutput::empty()).unwrap(),
            )),
            Arc::new(FixedClock::new(now())),
        );

        usecase
            .execute(RunCeremonyInput::new(
                ceremony_id(),
                definition,
                CeremonyContext::empty(),
                lease_owner(),
                lease_ttl(),
                "operator-1",
                AuditActorKind::Service,
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
            vec![
                AuditEventType::CeremonyInstanceStarted,
                AuditEventType::StepStarted,
                AuditEventType::StepCompleted,
                AuditEventType::TransitionApplied,
                AuditEventType::CeremonyCompleted,
            ],
            "a run left a different record than the verbs would have"
        );
    }

    /// Every record of a stream says what it correlates to and what
    /// caused it: the opening for the former, the record before it for
    /// the latter — the opening itself correlating to itself and caused
    /// by nothing.
    ///
    /// A run seals five records across four appends, so this covers
    /// causation within one batch (the move and the completion it
    /// produced) as well as across appends.
    #[tokio::test]
    async fn every_record_names_the_opening_and_the_record_before_it() {
        let definition = definition();
        let definitions = Arc::new(DefinitionRepositoryFake::new(definition.clone()));
        let instances = Arc::new(EventStoreFake::default());
        let (stream, store) = stream_over(instances);
        let usecase = RunCeremonyUseCase::new(
            definitions,
            stream,
            Arc::new(StepHandlerFake::succeeding(
                StepResult::completed(StepOutput::empty()).unwrap(),
            )),
            Arc::new(FixedClock::new(now())),
        );

        usecase
            .execute(RunCeremonyInput::new(
                ceremony_id(),
                definition,
                CeremonyContext::empty(),
                lease_owner(),
                lease_ttl(),
                "operator-1",
                AuditActorKind::Service,
            ))
            .await
            .unwrap();

        let records = store.records(&ceremony_id()).await;
        assert_eq!(records.len(), 5, "{records:?}");
        let opening = records[0].event_id().clone();
        assert_eq!(
            records[0].event_type(),
            AuditEventType::CeremonyInstanceStarted
        );
        assert_eq!(records[0].correlation_id(), Some(&opening));
        assert_eq!(records[0].causation_id(), None);
        for pair in records.windows(2) {
            let (previous, record) = (&pair[0], &pair[1]);
            assert_eq!(
                record.correlation_id(),
                Some(&opening),
                "{} does not correlate to the opening",
                record.event_id()
            );
            assert_eq!(
                record.causation_id(),
                Some(previous.event_id()),
                "{} is not caused by the record before it",
                record.event_id()
            );
        }
    }
}
