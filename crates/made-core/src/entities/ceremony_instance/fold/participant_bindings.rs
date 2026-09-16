use crate::entities::ceremony_events::ParticipantsBound;
use crate::entities::CeremonyInstance;

impl CeremonyInstance {
    /// Each binding replaces whoever held the seat, which is what
    /// seating a role again means.
    pub(super) fn apply_participants_bound(&mut self, bound: &ParticipantsBound) {
        for binding in &bound.bindings {
            self.participant_bindings
                .insert(binding.role_id().clone(), binding.clone());
            self.updated_at = binding.bound_at();
        }
    }
}
