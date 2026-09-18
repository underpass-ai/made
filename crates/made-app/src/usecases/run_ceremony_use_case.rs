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
use made_core::value_objects::{MaxParallel, StateExecution};

use super::ceremony_step_trace::CeremonyStepTrace;
use super::run_ceremony_input::RunCeremonyInput;
use super::run_ceremony_output::RunCeremonyOutput;
use super::PrepareCeremonyChildrenUseCase;

mod claimed_step;
mod concurrent_state;
mod executed_step;
mod run_step_output;
mod spawn_step;
mod steps;

use run_step_output::RunStepOutput;

#[cfg(test)]
mod aggregation_tests;
#[cfg(test)]
mod claim_visit_tests;
#[cfg(test)]
mod concurrent_tests;

/// Drives a declarative ceremony through its steps and transitions.
pub struct RunCeremonyUseCase {
    definitions: Arc<dyn CeremonyDefinitionRepositoryPort>,
    stream: Arc<SessionStream>,
    handler: Arc<dyn CeremonyStepHandlerPort>,
    clock: Arc<dyn ClockPort>,
    metrics: Arc<dyn MetricsRecorderPort>,
    max_parallel_ceiling: MaxParallel,
    children: Option<Arc<PrepareCeremonyChildrenUseCase>>,
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
            max_parallel_ceiling: MaxParallel::SERVER_MAX,
            children: None,
        }
    }

    /// Count driver refusals and blocked transitions that produce no event.
    /// Event subscribers record committed outcomes, durations and step status.
    #[must_use]
    pub fn with_metrics(mut self, metrics: Arc<dyn MetricsRecorderPort>) -> Self {
        self.metrics = metrics;
        self
    }

    /// Cap automatic fan-out independently of a definition's declared width.
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
        if definition
            .steps()
            .values()
            .any(|step| step.spawn().is_some())
            && self.children.is_none()
        {
            return Err(DomainError::InvariantViolated {
                reason: "child-spawning ceremony requires the child orchestrator",
            });
        }
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

        let max_state_iterations = definition
            .states()
            .values()
            .map(|state| {
                state
                    .repeat_policy()
                    .map_or(1, |repeat| repeat.max_iterations().get() as usize)
            })
            .max()
            .unwrap_or(1);
        let total_transition_allowance = definition
            .max_transitions()
            .map_or(usize::MAX, |limit| limit.get() as usize);
        let bounce_transition_allowance = definition.max_bounces().map_or(usize::MAX, |limit| {
            (limit.get() as usize).saturating_mul(definition.transitions().len())
        });
        let bounded_transition_allowance = total_transition_allowance
            .min(bounce_transition_allowance)
            .min(
                if total_transition_allowance == usize::MAX
                    && bounce_transition_allowance == usize::MAX
                {
                    definition.transitions().len()
                } else {
                    usize::MAX
                },
            );
        // One pass can either start the next state iteration or apply
        // one transition. Capped cycles may visit a state repeatedly,
        // so a graph-size-only ceiling would stop before the declared
        // budget did. Saturation keeps hostile but valid authoring
        // values finite on this platform.
        let max_iterations = bounded_transition_allowance
            .saturating_add(1)
            .saturating_mul(max_state_iterations)
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
            let state_execution = definition
                .state(&state_id)
                .ok_or(DomainError::NotFound {
                    what: "ceremony_instance.current_state",
                })?
                .execution();
            if state_execution == StateExecution::Concurrent {
                let concurrent = self
                    .run_concurrent_state(
                        &definition,
                        session,
                        actor_kind,
                        &state_id,
                        state_iteration,
                        &lease_owner_id,
                        lease_ttl,
                        step_traces.len(),
                    )
                    .await?;
                session = concurrent.session;
                step_traces.extend(concurrent.step_traces);
                if concurrent.state_iteration_changed {
                    continue 'driver;
                }
                if concurrent.step_failed {
                    return Err(DomainError::InvariantViolated {
                        reason: "ceremony step did not complete successfully",
                    });
                }
            } else {
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
                        // What was said so far, folded from the stream the
                        // steps before this one sealed.
                        let transcript = ceremony_transcript_projection::transcript(
                            &self.stream.records(&id).await?,
                        );
                        let RunStepOutput {
                            session: moved_on,
                            step_id: _,
                            role_id,
                            state_visit: executed_state_visit,
                            state_iteration: executed_state_iteration,
                            iteration,
                            attempt,
                            result: step_result,
                        } = self
                            .run_step(
                                &definition,
                                session,
                                actor_kind,
                                &step_id,
                                &lease_owner_id,
                                lease_ttl,
                                step_traces.len(),
                                transcript,
                            )
                            .await?;
                        session = moved_on;
                        step_traces.push(
                            CeremonyStepTrace::for_coordinates(
                                state_id.clone(),
                                executed_state_iteration,
                                step_id.clone(),
                                role_id,
                                iteration,
                                attempt,
                                step_result.status(),
                                step_result.output().clone(),
                            )
                            .with_state_visit(executed_state_visit),
                        );
                        if !step_result.is_success() {
                            return Err(DomainError::InvariantViolated {
                                reason: "ceremony step did not complete successfully",
                            });
                        }
                        if session
                            .instance
                            .step_repeat_limit_reached(&definition, &step_id)
                        {
                            self.metrics.record_ceremony_outcome(
                                &ceremony_name,
                                CeremonyOutcome::RepeatLimit,
                            );
                            return Err(DomainError::InvariantViolated {
                                reason: "ceremony step repeat limit exhausted",
                            });
                        }
                        if session.instance.current_state_iteration() != state_iteration {
                            continue 'driver;
                        }
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
                .or_else(|| {
                    definition
                        .available_transitions(&state_id)
                        .find(|transition| {
                            session
                                .instance
                                .transition_requirements_are_satisfied(&definition, transition)
                        })
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

    use made_core::entities::ceremony_events::ContextWritten;
    use made_core::entities::{CeremonyDefinition, CeremonyEvent};
    use made_core::error::DomainError;
    use made_core::ports::CeremonyDefinitionRepositoryPort;
    use made_core::value_objects::{
        Attributes, AuditActorKind, AuditEventType, CeremonyContext, CeremonyGuard, CeremonyName,
        CeremonyRole, CeremonyState, CeremonyStep, CeremonyTranscript, CeremonyTransition,
        CeremonyVersion, ContextKey, ContextPatch, DynamicRoleBinding, GuardCondition, GuardName,
        MaxBounces, MaxTransitions, RetryPolicy, RoleAction, RoleId, Specialty, StateId,
        StateIteration, StepAttempt, StepHandlerConfig, StepHandlerKind, StepId, StepIteration,
        StepOutput, StepResult, StepStatus, TransitionTrigger,
    };
    use serde_json::json;

    use super::*;
    use crate::usecases::ceremony_test_support::{
        a_memory, approval_definition, ceremony_id, child_spawning_definition, definition,
        lease_owner, lease_ttl, nested_multi_iteration_definition, nested_repeating_definition,
        now, repeating_definition, resolver_with, review_child_definition, started_instance,
        state_repeating_definition, step_id, stream, stream_over, stream_overtaken_once,
        two_step_definition, DefinitionRepositoryFake, EventStoreFake, FixedClock,
        PublicationsFake, SequenceStepHandlerFake, StepHandlerFake,
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
    async fn one_shot_spawn_without_an_orchestrator_is_refused_before_opening() {
        let definition = child_spawning_definition();
        let definitions = Arc::new(DefinitionRepositoryFake::default());
        let store = Arc::new(EventStoreFake::default());
        let handler = Arc::new(StepHandlerFake::succeeding(
            StepResult::completed(StepOutput::empty()).unwrap(),
        ));
        let usecase = RunCeremonyUseCase::new(
            definitions,
            stream(store.clone()),
            handler.clone(),
            Arc::new(FixedClock::new(now())),
        );

        let error = usecase
            .execute(RunCeremonyInput::new(
                ceremony_id(),
                definition,
                CeremonyContext::empty(),
                lease_owner(),
                lease_ttl(),
                "operator",
                AuditActorKind::Service,
            ))
            .await
            .unwrap_err();

        assert!(error.to_string().contains("child orchestrator"));
        assert!(!store.exists(&ceremony_id()).await);
        assert!(handler.requests().await.is_empty());
    }

    #[tokio::test]
    async fn one_shot_spawn_uses_the_configured_orchestrator_and_reaches_terminal() {
        let definition = child_spawning_definition();
        let definitions = Arc::new(DefinitionRepositoryFake::default());
        let publications = Arc::new(PublicationsFake::default());
        publications.seed(review_child_definition()).await;
        let store = Arc::new(EventStoreFake::default());
        let stream = stream(store.clone());
        let children = Arc::new(PrepareCeremonyChildrenUseCase::new(
            resolver_with(definitions.clone(), publications.clone()),
            publications,
            stream.clone(),
            Arc::new(FixedClock::new(now())),
            a_memory(),
        ));
        let handler = Arc::new(StepHandlerFake::failing(DomainError::InvariantViolated {
            reason: "spawn steps must not reach a handler",
        }));
        let usecase = RunCeremonyUseCase::new(
            definitions,
            stream,
            handler.clone(),
            Arc::new(FixedClock::new(now())),
        )
        .with_child_orchestrator(children);

        let output = usecase
            .execute(RunCeremonyInput::new(
                ceremony_id(),
                definition.clone(),
                CeremonyContext::empty(),
                lease_owner(),
                lease_ttl(),
                "operator",
                AuditActorKind::Service,
            ))
            .await
            .unwrap();

        assert!(output.instance().is_completed(&definition));
        assert!(handler.requests().await.is_empty());
        let group = output.instance().child_groups().values().next().unwrap();
        let child = group.plan().children()[0].child_id();
        assert!(
            store.exists(child).await,
            "the planned child was not opened"
        );
    }

    fn cyclic_definition(
        max_transitions: Option<MaxTransitions>,
        max_bounces: Option<MaxBounces>,
    ) -> CeremonyDefinition {
        let state = StateId::new("LOOP").unwrap();
        let again = TransitionTrigger::new("again").unwrap();
        CeremonyDefinition::new_with_transition_budgets(
            CeremonyName::new("bounded_driver").unwrap(),
            CeremonyVersion::v1(),
            None,
            Vec::new(),
            Vec::new(),
            vec![CeremonyState::initial(state.clone())],
            vec![CeremonyTransition::new(state.clone(), state, again.clone(), Vec::new()).unwrap()],
            Vec::new(),
            Vec::new(),
            vec![CeremonyRole::new(
                RoleId::new("DRIVER").unwrap(),
                vec![RoleAction::transition(again)],
            )
            .unwrap()],
            max_transitions,
            max_bounces,
        )
        .unwrap()
    }

    fn dynamic_role_definition() -> CeremonyDefinition {
        let state = StateId::new("REVIEWING").unwrap();
        let done = StateId::new("DONE").unwrap();
        let step = CeremonyStep::new(
            step_id(),
            state.clone(),
            StepHandlerKind::new("multiagent_round").unwrap(),
            StepHandlerConfig::empty(),
            RetryPolicy::single_attempt(),
            None,
        )
        .with_dynamic_role_binding(
            DynamicRoleBinding::new(
                ContextKey::new("next_role").unwrap(),
                [
                    RoleId::new("REVIEWER_A").unwrap(),
                    RoleId::new("REVIEWER_B").unwrap(),
                ],
            )
            .unwrap(),
        );
        let guard = CeremonyGuard::new(
            GuardName::new("reviewed").unwrap(),
            GuardCondition::StepStatus {
                step_id: step.id().clone(),
                status: StepStatus::Completed,
            },
        );
        let transition = CeremonyTransition::new(
            state.clone(),
            done.clone(),
            TransitionTrigger::new("reviewed").unwrap(),
            vec![guard.name().clone()],
        )
        .unwrap();
        let reviewer_a = CeremonyRole::new(
            RoleId::new("REVIEWER_A").unwrap(),
            [RoleAction::step(step.id().clone())],
        )
        .unwrap();
        let reviewer_b = CeremonyRole::new(
            RoleId::new("REVIEWER_B").unwrap(),
            [RoleAction::step(step.id().clone())],
        )
        .unwrap();
        let driver = CeremonyRole::new(
            RoleId::new("DRIVER").unwrap(),
            [RoleAction::transition(transition.trigger().clone())],
        )
        .unwrap();
        CeremonyDefinition::new(
            CeremonyName::new("dynamic_review").unwrap(),
            CeremonyVersion::v1(),
            None,
            Vec::new(),
            Vec::new(),
            vec![CeremonyState::initial(state), CeremonyState::terminal(done)],
            vec![transition],
            vec![step],
            vec![guard],
            vec![reviewer_a, reviewer_b, driver],
        )
        .unwrap()
    }

    fn dynamic_role_context(role: &RoleId) -> CeremonyContext {
        CeremonyContext::new(
            Attributes::new(BTreeMap::from([(
                "next_role".to_owned(),
                json!(role.as_str()),
            )]))
            .unwrap(),
        )
    }

    async fn bounded_driver_refusal(definition: CeremonyDefinition) -> DomainError {
        let usecase = RunCeremonyUseCase::new(
            Arc::new(DefinitionRepositoryFake::default()),
            stream(Arc::new(EventStoreFake::default())),
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
            .unwrap_err()
    }

    #[tokio::test]
    async fn one_shot_driver_surfaces_total_transition_budget_refusal() {
        let error = bounded_driver_refusal(cyclic_definition(
            Some(MaxTransitions::new(1).unwrap()),
            None,
        ))
        .await;
        assert!(matches!(
            error,
            DomainError::InvariantViolated {
                reason: "ceremony transition limit exhausted"
            }
        ));
    }

    #[tokio::test]
    async fn one_shot_driver_surfaces_exact_edge_budget_refusal() {
        let error =
            bounded_driver_refusal(cyclic_definition(None, Some(MaxBounces::new(1).unwrap())))
                .await;
        assert!(matches!(
            error,
            DomainError::InvariantViolated {
                reason: "ceremony transition bounce limit exhausted"
            }
        ));
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
    async fn exhausted_step_repeat_cannot_be_reset_by_state_repeat() {
        let definition = nested_repeating_definition();
        let definitions = Arc::new(DefinitionRepositoryFake::default());
        let instances = Arc::new(EventStoreFake::default());
        let (stream, store) = stream_over(instances.clone());
        let output = StepOutput::new(
            Attributes::new(BTreeMap::from([
                ("ready".to_owned(), json!(false)),
                ("state_ready".to_owned(), json!(true)),
            ]))
            .unwrap(),
        );
        let handler = Arc::new(StepHandlerFake::succeeding(
            StepResult::completed(output).unwrap(),
        ));
        let usecase = RunCeremonyUseCase::new(
            definitions,
            stream,
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
                reason: "ceremony step repeat limit exhausted"
            }
        ));
        let saved = instances.saved(&ceremony_id()).await;
        assert_eq!(saved.current_state_iteration().get(), 1);
        assert!(!saved.state_work_is_complete(&definition));
        assert!(
            !saved.state_repeat_permits_transition(&definition),
            "a true state predicate cannot waive an unmet nested step repeat"
        );
        assert!(!store
            .facts()
            .await
            .iter()
            .any(|fact| fact.event.event_type() == AuditEventType::StateIterationStarted));
    }

    #[tokio::test]
    async fn state_step_and_attempt_coordinates_advance_and_reset_independently() {
        let definition = nested_multi_iteration_definition();
        let definitions = Arc::new(DefinitionRepositoryFake::default());
        let instances = Arc::new(EventStoreFake::default());
        let results = [(false, false), (true, false), (false, false), (true, true)].map(
            |(ready, accepted)| {
                StepResult::completed(StepOutput::new(
                    Attributes::new(BTreeMap::from([
                        ("ready".to_owned(), json!(ready)),
                        ("accepted".to_owned(), json!(accepted)),
                    ]))
                    .unwrap(),
                ))
                .unwrap()
            },
        );
        let handler = Arc::new(SequenceStepHandlerFake::new(results));
        let usecase = RunCeremonyUseCase::new(
            definitions,
            stream(instances),
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

        assert_eq!(
            output
                .step_traces()
                .iter()
                .map(|trace| (
                    trace.state_iteration().get(),
                    trace.iteration().get(),
                    trace.attempt().get()
                ))
                .collect::<Vec<_>>(),
            vec![(1, 1, 1), (1, 2, 1), (2, 1, 1), (2, 2, 1)]
        );
        let history = output.instance().step_record_history(&step_id());
        assert_eq!(
            history
                .iter()
                .map(|record| (record.state_iteration().get(), record.iteration().get()))
                .collect::<Vec<_>>(),
            vec![(1, 1), (1, 2), (2, 1)]
        );
        let current = output.instance().step_record(&step_id()).unwrap();
        assert_eq!(
            (current.state_iteration().get(), current.iteration().get()),
            (2, 2)
        );
    }

    #[tokio::test]
    async fn claim_retry_uses_the_dynamic_role_from_the_winning_context() {
        let definition = dynamic_role_definition();
        let winning_role = RoleId::new("REVIEWER_B").unwrap();
        let winning_specialty = Specialty::new("security-review").unwrap();
        let mut instance = CeremonyInstance::start(
            ceremony_id(),
            &definition,
            dynamic_role_context(&RoleId::new("REVIEWER_A").unwrap()),
            now(),
        )
        .unwrap();
        instance
            .bind_participant(
                &definition,
                winning_role.clone(),
                winning_specialty.clone(),
                now(),
            )
            .unwrap();

        let store = Arc::new(EventStoreFake::default());
        store.save(&instance).await.unwrap();
        let context_changed = CeremonyEvent::ContextWritten(ContextWritten {
            state_visit: None,
            step_id: step_id(),
            state_iteration: StateIteration::FIRST,
            iteration: StepIteration::FIRST,
            attempt: StepAttempt::FIRST,
            patch: ContextPatch::new(BTreeMap::from([(
                ContextKey::new("next_role").unwrap(),
                json!(winning_role.as_str()),
            )]))
            .unwrap(),
            written_at: now(),
        });
        let overtaking_fact = session_facts::fact(
            &instance,
            context_changed,
            session_facts::seat(&winning_role, AuditActorKind::Agent).unwrap(),
            now(),
        )
        .unwrap();
        let handler = Arc::new(StepHandlerFake::succeeding(
            StepResult::completed(StepOutput::empty()).unwrap(),
        ));
        let usecase = RunCeremonyUseCase::new(
            Arc::new(DefinitionRepositoryFake::new(definition.clone())),
            stream_overtaken_once(store.clone(), overtaking_fact),
            handler.clone(),
            Arc::new(FixedClock::new(now())),
        );
        let loaded = usecase.stream.load(&ceremony_id()).await.unwrap();

        let outcome = usecase
            .run_step(
                &definition,
                loaded,
                AuditActorKind::Agent,
                &step_id(),
                &lease_owner(),
                lease_ttl(),
                0,
                CeremonyTranscript::empty(),
            )
            .await
            .unwrap();

        assert_eq!(outcome.role_id, winning_role);
        let claimed = outcome.session.instance.step_record(&step_id()).unwrap();
        assert_eq!(claimed.claimed_role(), Some(&winning_role));
        let trace = CeremonyStepTrace::for_coordinates(
            StateId::new("REVIEWING").unwrap(),
            outcome.state_iteration,
            step_id(),
            outcome.role_id,
            outcome.iteration,
            outcome.attempt,
            outcome.result.status(),
            outcome.result.output().clone(),
        )
        .with_state_visit(outcome.state_visit);
        assert_eq!(trace.role_id(), &winning_role);

        let requests = handler.requests().await;
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].role_id(), Some(&winning_role));
        assert_eq!(requests[0].bound_specialty(), Some(&winning_specialty));
        let step_records = store
            .records(&ceremony_id())
            .await
            .into_iter()
            .filter(|record| {
                matches!(
                    record.event_type(),
                    AuditEventType::StepStarted | AuditEventType::StepCompleted
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(step_records.len(), 2);
        assert!(step_records
            .iter()
            .all(|record| record.actor().role_id() == Some(&winning_role)));
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
