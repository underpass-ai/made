use crate::entities::protocol_events::{
    LateStepResultObserved, ProtocolCancelled, ProtocolDeadlineExceeded, ProtocolPaused,
    ProtocolResumed, StateDeadlineExceeded, StepDeadlineExceeded,
};
use crate::entities::ProtocolInstance;
use crate::value_objects::ProtocolEndReason;

impl ProtocolInstance {
    pub(super) fn apply_protocol_paused(&mut self, event: &ProtocolPaused) {
        self.lifecycle.pause(event.reason.clone(), event.paused_at);
        self.updated_at = event.paused_at;
    }
    pub(super) fn apply_protocol_resumed(&mut self, event: &ProtocolResumed) {
        self.lifecycle.resume(event.resumed_at);
        self.updated_at = event.resumed_at;
    }
    pub(super) fn apply_protocol_cancelled(&mut self, event: &ProtocolCancelled) {
        self.lifecycle.end(
            ProtocolEndReason::Cancelled,
            Some(event.reason.clone()),
            event.cancelled_at,
        );
        self.updated_at = event.cancelled_at;
    }
    pub(super) fn apply_protocol_deadline_exceeded(&mut self, event: &ProtocolDeadlineExceeded) {
        self.lifecycle
            .end(ProtocolEndReason::ProtocolDeadline, None, event.observed_at);
        self.updated_at = event.observed_at;
    }
    pub(super) fn apply_state_deadline_exceeded(&mut self, event: &StateDeadlineExceeded) {
        self.lifecycle
            .end(ProtocolEndReason::StateDeadline, None, event.observed_at);
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
