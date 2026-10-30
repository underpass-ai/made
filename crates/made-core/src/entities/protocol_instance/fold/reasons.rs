use crate::entities::protocol_events::ReasonAsserted;
use crate::entities::ProtocolInstance;

impl ProtocolInstance {
    pub(super) fn apply_reason_asserted(&mut self, asserted: &ReasonAsserted) {
        self.updated_at = asserted.reason.asserted_at();
        self.reasons.push(asserted.reason.clone());
    }
}
