use serde::{Deserialize, Serialize};

use crate::value_objects::{CeremonyInterventionId, InterventionDeliveryAck};

/// A live agent said it had been handed an intervention.
///
/// The one delivery fact a ceremony's stream holds. It is a domain
/// fact and not a transport one because somebody observed something:
/// a named process generation, at a named time, said what it saw. The
/// offer that produced it, the lease it was handed under and every
/// attempt that failed first live in the delivery ledger, which is
/// where an operator looks for "why has nobody answered".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterventionDeliveryAcknowledged {
    pub intervention_id: CeremonyInterventionId,
    pub ack: InterventionDeliveryAck,
}
