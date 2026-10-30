use time::OffsetDateTime;

use crate::value_objects::{
    InterventionDeliveryPolicy, ProtocolInterventionContent, ProtocolInterventionId,
    ProtocolInterventionIntent, ProtocolInterventionKind, ProtocolInterventionProvenance,
    ProtocolInterventionTarget, RoleId, SupervisorPrincipal,
};

/// Ask the table for something.
///
/// `provenance` is present when the item was selected out of an
/// earlier intervention's response. `supervisor` is present when the
/// asker holds no seat: the aggregate then takes `role_id` to be the
/// seat derived from that principal and checks nothing else against
/// the definition, because asking is not one of the actions a role
/// grants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestIntervention {
    pub intervention_id: ProtocolInterventionId,
    pub role_id: RoleId,
    pub kind: ProtocolInterventionKind,
    pub target: ProtocolInterventionTarget,
    pub content: ProtocolInterventionContent,
    pub provenance: Option<ProtocolInterventionProvenance>,
    pub intent: Option<ProtocolInterventionIntent>,
    pub delivery: Option<InterventionDeliveryPolicy>,
    pub supervisor: Option<SupervisorPrincipal>,
    pub now: OffsetDateTime,
}
