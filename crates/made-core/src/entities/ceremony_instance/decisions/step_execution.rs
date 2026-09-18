use crate::entities::ceremony_commands::{ApplyStepResult, StartStep};
use crate::entities::ceremony_events::{
    ContextWritten, LateStepResultObserved, StateIterationStarted, StepCompleted, StepFailed,
    StepStarted,
};
use crate::entities::{CeremonyDefinition, CeremonyEvent, CeremonyInstance};
use crate::error::DomainError;
use crate::value_objects::{
    LateStepResult, StateExecution, StepAttempt, StepClaimFence, StepDeadline, StepExecutionRecord,
    StepStatus,
};

impl CeremonyInstance {
    /// A step may be taken when its state is current, its lease is
    /// free or expired, its retry policy has attempts left and its
    /// idempotency key is new. The event names the seat that took it:
    /// the one the command declares, once the definition allows it
    /// the step, or the seat the definition assigns to the step when
    /// the engine took it.
    pub(super) fn decide_start_step(
        &self,
        command: &StartStep,
        definition: &CeremonyDefinition,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        self.require_definition(definition)?;
        self.require_admits_new_work("start_step")?;
        if self.is_terminal(definition) || self.is_ended() {
            return Err(DomainError::InvariantViolated {
                reason: "terminal ceremony instances cannot start steps",
            });
        }

        let step = definition
            .step(&command.step_id)
            .ok_or(DomainError::NotFound {
                what: "ceremony_instance.step",
            })?;
        if step.state_id() != &self.current_state {
            return Err(DomainError::InvalidTransition {
                from: "ceremony_instance.current_state",
                to: "ceremony_step.state",
            });
        }

        let record = self
            .step_records
            .get(&command.step_id)
            .ok_or(DomainError::NotFound {
                what: "ceremony_instance.step_record",
            })?;
        if !record.can_be_started_at(command.now) {
            return Err(DomainError::InvariantViolated {
                reason: "step lease is still active",
            });
        }
        let attempt = next_attempt_for_start(record)?;
        if !step.retry_policy().allows_attempt(attempt) {
            return Err(DomainError::InvariantViolated {
                reason: "step retry policy exhausted",
            });
        }
        let (started_by, dynamic) =
            self.resolve_step_role(definition, &command.step_id, command.role_id.as_ref())?;
        let concurrent_state = definition
            .state(&self.current_state)
            .is_some_and(|state| state.execution() == StateExecution::Concurrent);
        let mixed_concurrent_state = concurrent_state
            && definition
                .steps_for_state(&self.current_state)
                .any(|candidate| candidate.dynamic_role_binding().is_some());
        let alternate_static_role = !dynamic
            && concurrent_state
            && definition.role_id_for_step(&command.step_id)? != started_by;
        let sealed_role = (!dynamic && (mixed_concurrent_state || alternate_static_role))
            .then(|| started_by.clone());
        if !self.resolved_step_is_claimable_at(
            &command.step_id,
            definition,
            command.now,
            command.max_parallel_ceiling,
        )? {
            return Err(DomainError::InvariantViolated {
                reason: "ceremony step is not claimable at the observed time and capacity",
            });
        }
        if self
            .idempotency_keys
            .contains(command.lease.idempotency_key())
        {
            return Err(DomainError::AlreadyExists {
                what: "ceremony_instance.idempotency_key",
            });
        }
        let claimed_role = dynamic
            .then(|| started_by.clone())
            .or_else(|| sealed_role.clone());
        let claimed_record =
            record
                .clone()
                .with_started(command.lease.clone(), attempt, claimed_role);
        let claim_fence = StepClaimFence::for_record(self.id(), &command.step_id, &claimed_record)?;
        let mut deadline_at = step
            .timeout()
            .map(|timeout| {
                super::start::checked_deadline(command.now, timeout.duration(), "step_deadline")
            })
            .transpose()?;
        if let Some(state) = self
            .state_deadline
            .as_ref()
            .map(crate::value_objects::StateDeadline::at)
        {
            deadline_at = Some(deadline_at.map_or(state, |step| step.min(state)));
        }
        if let Some(ceremony) = self
            .ceremony_deadline
            .map(crate::value_objects::CeremonyDeadline::at)
        {
            deadline_at = Some(deadline_at.map_or(ceremony, |step| step.min(ceremony)));
        }
        let deadline = deadline_at.map(|at| {
            StepDeadline::new(
                command.step_id.clone(),
                self.current_state_visit,
                self.current_state_iteration,
                record.iteration(),
                attempt,
                claim_fence,
                started_by.clone(),
                at,
            )
        });
        Ok(vec![CeremonyEvent::StepStarted(StepStarted {
            step_id: command.step_id.clone(),
            state_visit: Some(self.current_state_visit),
            state_iteration: Some(self.current_state_iteration),
            iteration: record.iteration(),
            attempt,
            lease: command.lease.clone(),
            started_by,
            role_from: dynamic.then(|| {
                step.dynamic_role_binding()
                    .expect("a dynamically resolved step has a binding")
                    .context_key()
                    .clone()
            }),
            sealed_role,
            deadline,
            started_at: command.now,
        })])
    }

    /// A result is filed against the in-progress record of a step in
    /// the current state. Whether a success reopens the step at the
    /// next iteration is the repeat policy's call, decided here and
    /// carried in the event so the fold needs no definition.
    pub(super) fn decide_apply_step_result(
        &self,
        command: &ApplyStepResult,
        definition: &CeremonyDefinition,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        self.require_definition(definition)?;
        if command.result.failure_kind().is_some() && command.result.status() != StepStatus::Failed
        {
            return Err(DomainError::InvariantViolated {
                reason: "only failed step results may carry a failure kind",
            });
        }
        let step = definition
            .step(&command.step_id)
            .ok_or(DomainError::NotFound {
                what: "ceremony_instance.step",
            })?;
        if let Some(existing) = self.late_step_results.get(&command.claim_fence) {
            return if existing.step_id() == &command.step_id && existing.result() == &command.result
            {
                Ok(Vec::new())
            } else {
                Err(DomainError::InvariantViolated {
                    reason: "late step claim already has a different step or observed result",
                })
            };
        }
        let retired = self
            .retired_deadline_claims
            .get(&command.claim_fence)
            .filter(|deadline| deadline.step_id() == &command.step_id);
        if self.is_ended() || retired.is_some() {
            let record = self
                .step_records
                .get(&command.step_id)
                .ok_or(DomainError::NotFound {
                    what: "ceremony_instance.step_record",
                })?;
            let live_matches = record.status() == StepStatus::InProgress
                && self
                    .step_claim_fence(&command.step_id)
                    .is_ok_and(|fence| fence == command.claim_fence);
            if !live_matches && retired.is_none() {
                return Err(DomainError::InvariantViolated {
                    reason: "step result claim fence is not current or retired by a deadline",
                });
            }
            let (state_visit, state_iteration, iteration, attempt) = retired.map_or(
                (
                    record.state_visit(),
                    record.state_iteration(),
                    record.iteration(),
                    record.attempt(),
                ),
                |deadline| {
                    (
                        deadline.state_visit(),
                        deadline.state_iteration(),
                        deadline.step_iteration(),
                        deadline.attempt(),
                    )
                },
            );
            let finished_by = retired.map_or_else(
                || {
                    record
                        .claimed_role()
                        .cloned()
                        .map_or_else(|| definition.role_id_for_step(&command.step_id), Ok)
                },
                |deadline| Ok(deadline.finished_by().clone()),
            )?;
            return Ok(vec![CeremonyEvent::LateStepResultObserved(
                LateStepResultObserved {
                    result: LateStepResult::new(
                        command.step_id.clone(),
                        state_visit,
                        state_iteration,
                        iteration,
                        attempt,
                        command.claim_fence.clone(),
                        command.result.clone(),
                        finished_by,
                    ),
                    observed_at: command.now,
                },
            )]);
        }
        if step.state_id() != &self.current_state {
            return Err(DomainError::InvalidTransition {
                from: "ceremony_instance.current_state",
                to: "ceremony_step.state",
            });
        }

        let record = self
            .step_records
            .get(&command.step_id)
            .ok_or(DomainError::NotFound {
                what: "ceremony_instance.step_record",
            })?;
        if record.status() != StepStatus::InProgress {
            return Err(DomainError::InvariantViolated {
                reason: "step result requires an in-progress step",
            });
        }

        self.require_step_claim_fence(&command.step_id, &command.claim_fence)?;

        let iteration = record.iteration();
        let attempt = record.attempt();
        let result = command.result.clone();
        if !result.is_success() {
            let finished_by = record
                .claimed_role()
                .cloned()
                .map_or_else(|| definition.role_id_for_step(&command.step_id), Ok)?;
            return Ok(vec![CeremonyEvent::StepFailed(StepFailed {
                step_id: command.step_id.clone(),
                state_visit: Some(self.current_state_visit),
                state_iteration: Some(self.current_state_iteration),
                iteration,
                attempt,
                result,
                finished_by,
                finished_at: command.now,
            })]);
        }

        let next_iteration = step
            .repeat_policy()
            .filter(|policy| !policy.is_satisfied(result.output()))
            .filter(|policy| policy.permits_another_iteration(iteration))
            .map(|_| iteration.next())
            .transpose()?;
        let finished_by = record
            .claimed_role()
            .cloned()
            .map_or_else(|| definition.role_id_for_step(&command.step_id), Ok)?;
        let patch = step.context_writes().resolve(result.output())?;
        let completed = CeremonyEvent::StepCompleted(StepCompleted {
            step_id: command.step_id.clone(),
            state_visit: Some(self.current_state_visit),
            state_iteration: Some(self.current_state_iteration),
            iteration,
            attempt,
            result,
            next_iteration,
            finished_by,
            finished_at: command.now,
        });
        let mut events = vec![completed];
        if let Some(patch) = patch {
            events.push(CeremonyEvent::ContextWritten(ContextWritten {
                step_id: command.step_id.clone(),
                state_visit: Some(self.current_state_visit),
                state_iteration: self.current_state_iteration,
                iteration,
                attempt,
                patch,
                written_at: command.now,
            }));
        }
        let mut projected = self.clone();
        projected.apply_all(&events);
        events.extend(projected.decide_state_iteration(definition, command.now)?);
        Ok(events)
    }

    fn decide_state_iteration(
        &self,
        definition: &CeremonyDefinition,
        now: time::OffsetDateTime,
    ) -> Result<Option<CeremonyEvent>, DomainError> {
        let Some(policy) = definition
            .state(&self.current_state)
            .and_then(|state| state.repeat_policy())
        else {
            return Ok(None);
        };
        if !self.state_work_is_complete(definition)
            || self.state_repeat_condition_is_satisfied(definition)
            || !policy.permits_another_iteration(self.current_state_iteration)
        {
            return Ok(None);
        }
        Ok(Some(CeremonyEvent::StateIterationStarted(
            StateIterationStarted {
                state_id: self.current_state.clone(),
                state_visit: Some(self.current_state_visit),
                state_iteration: self.current_state_iteration.next()?,
                step_ids: definition
                    .steps_for_state(&self.current_state)
                    .map(|step| step.id().clone())
                    .collect(),
                started_at: now,
            },
        )))
    }
}

fn next_attempt_for_start(record: &StepExecutionRecord) -> Result<StepAttempt, DomainError> {
    if matches!(record.status(), StepStatus::Failed | StepStatus::InProgress) {
        record.attempt().next()
    } else {
        Ok(record.attempt())
    }
}
