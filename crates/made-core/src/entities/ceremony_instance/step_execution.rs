use super::{
    CeremonyDefinition, CeremonyInstance, DomainError, OffsetDateTime, RoleAction, RoleId,
    StepAttempt, StepExecutionRecord, StepId, StepLease, StepResult, StepStatus,
};

impl CeremonyInstance {
    pub fn start_step_as(
        &mut self,
        definition: &CeremonyDefinition,
        role_id: &RoleId,
        step_id: &StepId,
        lease: StepLease,
        now: OffsetDateTime,
    ) -> Result<StepAttempt, DomainError> {
        self.require_role(definition, role_id, &RoleAction::step(step_id.clone()))?;
        self.start_step(definition, step_id, lease, now)
    }

    pub fn start_step(
        &mut self,
        definition: &CeremonyDefinition,
        step_id: &StepId,
        lease: StepLease,
        now: OffsetDateTime,
    ) -> Result<StepAttempt, DomainError> {
        self.require_definition(definition)?;
        if self.is_terminal(definition) {
            return Err(DomainError::InvariantViolated {
                reason: "terminal ceremony instances cannot start steps",
            });
        }

        let step = definition.step(step_id).ok_or(DomainError::NotFound {
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
            .get(step_id)
            .cloned()
            .ok_or(DomainError::NotFound {
                what: "ceremony_instance.step_record",
            })?;
        if !record.can_be_started_at(now) {
            return Err(DomainError::InvariantViolated {
                reason: "step lease is still active",
            });
        }

        let next_attempt = next_attempt_for_start(&record)?;
        if !step.retry_policy().allows_attempt(next_attempt) {
            return Err(DomainError::InvariantViolated {
                reason: "step retry policy exhausted",
            });
        }
        if !self
            .idempotency_keys
            .insert(lease.idempotency_key().clone())
        {
            return Err(DomainError::AlreadyExists {
                what: "ceremony_instance.idempotency_key",
            });
        }

        self.step_records
            .insert(step_id.clone(), record.with_started(lease, next_attempt));
        self.updated_at = now;
        Ok(next_attempt)
    }

    pub fn apply_step_result(
        &mut self,
        definition: &CeremonyDefinition,
        step_id: &StepId,
        result: StepResult,
        now: OffsetDateTime,
    ) -> Result<(), DomainError> {
        self.require_definition(definition)?;
        let step = definition.step(step_id).ok_or(DomainError::NotFound {
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
            .get(step_id)
            .cloned()
            .ok_or(DomainError::NotFound {
                what: "ceremony_instance.step_record",
            })?;
        if record.status() != StepStatus::InProgress {
            return Err(DomainError::InvariantViolated {
                reason: "step result requires an in-progress step",
            });
        }

        let finished = record.with_result(result);
        let repeat = step.repeat_policy().filter(|policy| {
            finished.status().is_success() && !policy.is_satisfied(finished.output())
        });
        if repeat.is_some_and(|policy| policy.permits_another_iteration(finished.iteration())) {
            let next_iteration = finished.iteration().next()?;
            self.step_record_history
                .entry(step_id.clone())
                .or_default()
                .push(finished);
            self.step_records.insert(
                step_id.clone(),
                StepExecutionRecord::pending_iteration(next_iteration),
            );
        } else {
            self.step_records.insert(step_id.clone(), finished);
        }
        self.updated_at = now;
        Ok(())
    }
}

fn next_attempt_for_start(record: &StepExecutionRecord) -> Result<StepAttempt, DomainError> {
    if matches!(record.status(), StepStatus::Failed | StepStatus::InProgress) {
        record.attempt().next()
    } else {
        Ok(record.attempt())
    }
}
