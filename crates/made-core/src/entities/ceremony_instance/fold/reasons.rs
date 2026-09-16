use crate::entities::ceremony_events::ReasonAsserted;
use crate::entities::CeremonyInstance;

impl CeremonyInstance {
    pub(super) fn apply_reason_asserted(&mut self, asserted: &ReasonAsserted) {
        self.updated_at = asserted.reason.asserted_at();
        self.reasons.push(asserted.reason.clone());
    }
}
