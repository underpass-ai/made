use crate::entities::ceremony_events::{
    StateIterationStarted, StepCompleted, StepFailed, StepStarted,
};
use crate::entities::CeremonyInstance;
use crate::value_objects::{StepExecutionRecord, StepId};

impl CeremonyInstance {
    /// The lease's idempotency key is taken and the record goes in
    /// progress at the attempt the event assigned.
    pub(super) fn apply_step_started(&mut self, started: &StepStarted) {
        self.idempotency_keys
            .insert(started.lease.idempotency_key().clone());
        let record = self.take_step_record(&started.step_id);
        self.step_records.insert(
            started.step_id.clone(),
            record.with_started(
                started.lease.clone(),
                started.attempt,
                started
                    .role_from
                    .as_ref()
                    .map(|_| started.started_by.clone())
                    .or_else(|| started.sealed_role.clone()),
            ),
        );
        self.updated_at = started.started_at;
    }

    /// A completion with a next iteration archives the finished
    /// record and reopens the step there; without one, the finished
    /// record is the step's final one.
    pub(super) fn apply_step_completed(&mut self, completed: &StepCompleted) {
        let finished = self
            .take_step_record(&completed.step_id)
            .with_result(completed.result.clone());
        match completed.next_iteration {
            Some(next_iteration) => {
                let state_iteration = finished.state_iteration();
                let state_visit = finished.state_visit();
                self.step_record_history
                    .entry(completed.step_id.clone())
                    .or_default()
                    .push(finished);
                self.step_records.insert(
                    completed.step_id.clone(),
                    StepExecutionRecord::pending_coordinates(state_iteration, next_iteration)
                        .with_state_visit(state_visit),
                );
            }
            None => {
                self.step_records
                    .insert(completed.step_id.clone(), finished);
            }
        }
        self.updated_at = completed.finished_at;
    }

    pub(super) fn apply_step_failed(&mut self, failed: &StepFailed) {
        let finished = self
            .take_step_record(&failed.step_id)
            .with_result(failed.result.clone());
        self.step_records.insert(failed.step_id.clone(), finished);
        self.updated_at = failed.finished_at;
    }

    pub(super) fn apply_state_iteration_started(&mut self, started: &StateIterationStarted) {
        self.current_state_iteration = started.state_iteration;
        for step_id in &started.step_ids {
            let finished = self.take_step_record(step_id);
            self.step_record_history
                .entry(step_id.clone())
                .or_default()
                .push(finished);
            self.step_records.insert(
                step_id.clone(),
                StepExecutionRecord::pending_state_iteration(started.state_iteration)
                    .with_state_visit(started.state_visit()),
            );
        }
        self.updated_at = started.started_at;
    }

    /// The record a step event is applied to. Every step the opening
    /// declared has one; a step it did not is opened pending here so
    /// the fold stays total.
    fn take_step_record(&mut self, step_id: &StepId) -> StepExecutionRecord {
        self.step_records
            .remove(step_id)
            .unwrap_or_else(StepExecutionRecord::pending)
    }
}
