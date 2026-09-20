//! What a host says it is doing, or has done, about one item.

use made_core::value_objects::{
    HostAgentIncarnation, HostDeliveryId, HostDeliveryLease, IntegratorBindingId, IntegratorFence,
};

use super::IntegratorAcknowledgement;

/// One acknowledgement of one delivery.
///
/// The lease travels with it because the ledger's exclusion is what
/// keeps two hosts from answering the same item, and the fence because
/// a host that was replaced must not be able to close the work of the
/// one that replaced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcknowledgeIntegratorAttentionInput {
    pub binding_id: IntegratorBindingId,
    pub incarnation: HostAgentIncarnation,
    pub fence: IntegratorFence,
    pub delivery_id: HostDeliveryId,
    pub lease: HostDeliveryLease,
    pub outcome: IntegratorAcknowledgement,
}
