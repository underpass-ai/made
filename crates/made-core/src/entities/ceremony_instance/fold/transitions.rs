use crate::entities::ceremony_events::{CeremonyCompleted, TransitionApplied};
use crate::entities::CeremonyInstance;

impl CeremonyInstance {
    /// The record is pushed as it is and the current state read off
    /// it. Whether the destination is terminal is not decided here:
    /// the completion event sealed with the move says so.
    pub(super) fn apply_transition_applied(&mut self, applied: &TransitionApplied) {
        self.current_state = applied.transition.to_state().clone();
        self.transitions.push(applied.transition.clone());
        self.updated_at = applied.transition.applied_at();
    }

    pub(super) fn apply_ceremony_completed(&mut self, completed: &CeremonyCompleted) {
        self.completed_at = Some(completed.completed_at);
        self.updated_at = completed.completed_at;
    }
}
