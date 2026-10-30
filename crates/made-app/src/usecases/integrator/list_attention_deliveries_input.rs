//! What an operator asks when it wants to see the loop's paperwork.

use made_core::ports::HostDeliveryPageLimit;
use made_core::value_objects::{
    HostDeliveryId, HostDeliveryStateKind, IntegratorBindingId, ProtocolId,
};

/// A page of what one integrator has been offered.
///
/// Either a binding or a protocol narrows it, and both may be given.
/// Neither is also allowed: an operator looking at a deployment that
/// is misbehaving does not yet know which binding to ask about, and
/// making them guess is how the useful question goes unasked.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ListAttentionDeliveriesInput {
    pub binding_id: Option<IntegratorBindingId>,
    pub protocol_id: Option<ProtocolId>,
    pub state: Option<HostDeliveryStateKind>,
    pub limit: HostDeliveryPageLimit,
    pub cursor: Option<HostDeliveryId>,
}
