use crate::entities::ceremony_events::{CeremonyCompleted, TransitionApplied};
use crate::entities::CeremonyInstance;
use crate::value_objects::{StateIteration, StepExecutionRecord, StepStatus};

impl CeremonyInstance {
    /// New transitions seal the exact destination reset; historical moves
    /// without it retain their old fold. Whether the target is terminal is
    /// carried by the completion event in the same append.
    pub(super) fn apply_transition_applied(&mut self, applied: &TransitionApplied) {
        if let Some(destination) = &applied.destination {
            self.current_state_visit = destination.state_visit;
            for step_id in &destination.step_ids {
                if let Some(previous) = self.step_records.remove(step_id) {
                    if previous.status() != StepStatus::Pending {
                        self.step_record_history
                            .entry(step_id.clone())
                            .or_default()
                            .push(previous);
                    }
                }
                self.step_records.insert(
                    step_id.clone(),
                    StepExecutionRecord::pending().with_state_visit(destination.state_visit),
                );
            }
        }
        self.current_state = applied.transition.to_state().clone();
        self.current_state_iteration = StateIteration::FIRST;
        self.transitions.push(applied.transition.clone());
        self.updated_at = applied.transition.applied_at();
    }

    pub(super) fn apply_ceremony_completed(&mut self, completed: &CeremonyCompleted) {
        self.completed_at = Some(completed.completed_at);
        self.updated_at = completed.completed_at;
    }
}
