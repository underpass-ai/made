use crate::entities::ceremony_events::{
    CeremonyCancelled, CeremonyDeadlineExceeded, CeremonyPaused, CeremonyResumed,
    LateStepResultObserved, StateDeadlineExceeded, StepDeadlineExceeded,
};
use crate::entities::CeremonyInstance;
use crate::value_objects::CeremonyEndReason;

impl CeremonyInstance {
    pub(super) fn apply_ceremony_paused(&mut self, event: &CeremonyPaused) {
        self.lifecycle.pause(event.reason.clone(), event.paused_at);
        self.updated_at = event.paused_at;
    }
    pub(super) fn apply_ceremony_resumed(&mut self, event: &CeremonyResumed) {
        self.lifecycle.resume(event.resumed_at);
        self.updated_at = event.resumed_at;
    }
    pub(super) fn apply_ceremony_cancelled(&mut self, event: &CeremonyCancelled) {
        self.lifecycle.end(
            CeremonyEndReason::Cancelled,
            Some(event.reason.clone()),
            event.cancelled_at,
        );
        self.updated_at = event.cancelled_at;
    }
    pub(super) fn apply_ceremony_deadline_exceeded(&mut self, event: &CeremonyDeadlineExceeded) {
        self.lifecycle
            .end(CeremonyEndReason::CeremonyDeadline, None, event.observed_at);
        self.updated_at = event.observed_at;
    }
    pub(super) fn apply_state_deadline_exceeded(&mut self, event: &StateDeadlineExceeded) {
        self.lifecycle
            .end(CeremonyEndReason::StateDeadline, None, event.observed_at);
        self.updated_at = event.observed_at;
    }
    pub(super) fn apply_step_deadline_exceeded(&mut self, event: &StepDeadlineExceeded) {
        let step_id = event.deadline.step_id().clone();
        if let Some(record) = self.step_records.remove(&step_id) {
            self.step_records
                .insert(step_id.clone(), record.with_result(event.result.clone()));
        }
        self.step_deadlines.remove(&step_id);
        self.retired_deadline_claims
            .insert(event.deadline.claim_fence().clone(), event.deadline.clone());
        self.updated_at = event.observed_at;
    }
    pub(super) fn apply_late_step_result_observed(&mut self, event: &LateStepResultObserved) {
        self.late_step_results
            .insert(event.result.claim_fence().clone(), event.result.clone());
        self.updated_at = event.observed_at;
    }
}
