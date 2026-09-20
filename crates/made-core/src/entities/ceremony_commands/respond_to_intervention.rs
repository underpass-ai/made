use time::OffsetDateTime;

use crate::value_objects::{
    CeremonyInterventionContent, CeremonyInterventionId, DeliveryRecipient, HostDeliveryId, RoleId,
};

/// Answer an open intervention from a seat.
///
/// `executor` and `delivery_id` are present when the answer came from
/// an agent that was handed the item, and they are checked against the
/// item's target rather than trusted: an answer that names an offer
/// must be given by the process generation that was offered it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RespondToIntervention {
    pub intervention_id: CeremonyInterventionId,
    pub role_id: RoleId,
    pub content: CeremonyInterventionContent,
    pub executor: Option<DeliveryRecipient>,
    pub delivery_id: Option<HostDeliveryId>,
    pub now: OffsetDateTime,
}
