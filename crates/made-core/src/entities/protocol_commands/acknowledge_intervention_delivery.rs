use time::OffsetDateTime;

use crate::value_objects::{InterventionDeliveryAck, ProtocolInterventionId};

/// Seal that a named live agent saw a named offer of an intervention.
///
/// The only transport-adjacent command the aggregate takes, and it is
/// here because what it seals is a statement by the host, not an
/// attempt by the engine. Queueing, leasing and expiry stay in the
/// ledger, where a retry is bookkeeping rather than a new fact about
/// the protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcknowledgeInterventionDelivery {
    pub intervention_id: ProtocolInterventionId,
    pub ack: InterventionDeliveryAck,
    pub now: OffsetDateTime,
}
