use crate::entities::ceremony_events::{SuccessionCarried, SuccessorPlanned};
use crate::entities::CeremonyInstance;
use crate::value_objects::StepExecutionRecord;

impl CeremonyInstance {
    /// The predecessor now names the ceremony that replaces it.
    ///
    /// Nothing else about the session changes. It is still paused,
    /// still holds every record it held, and may still be cancelled;
    /// what it may no longer do is resume, and that rule lives where
    /// resume is decided rather than here.
    pub(super) fn apply_successor_planned(&mut self, planned: &SuccessorPlanned) {
        self.updated_at = planned.plan.planned_at();
        self.successor_plan = Some(planned.plan.clone());
    }

    /// The successor writes down what it was given.
    ///
    /// Each carried step becomes a completed record whose work is
    /// addressed to the predecessor's stream. The attempt stays at the
    /// first: this instance made no attempt at all, and claiming
    /// otherwise would read as a retry that never happened.
    pub(super) fn apply_succession_carried(&mut self, carried: &SuccessionCarried) {
        let state_iteration = self.current_state_iteration;
        let state_visit = self.current_state_visit;
        for evidence in &carried.carried {
            if !self
                .step_records
                .contains_key(evidence.successor_step_id())
            {
                continue;
            }
            self.step_records.insert(
                evidence.successor_step_id().clone(),
                StepExecutionRecord::carried(
                    evidence.output().clone(),
                    evidence.source().clone(),
                    state_iteration,
                    state_visit,
                ),
            );
        }
        self.updated_at = carried.carried_at;
    }
}
