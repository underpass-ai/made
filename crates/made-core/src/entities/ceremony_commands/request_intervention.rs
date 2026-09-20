use time::OffsetDateTime;

use crate::value_objects::{
    CeremonyInterventionContent, CeremonyInterventionId, CeremonyInterventionIntent,
    CeremonyInterventionKind, CeremonyInterventionProvenance, CeremonyInterventionTarget,
    InterventionDeliveryPolicy, RoleId, SupervisorPrincipal,
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
    pub intervention_id: CeremonyInterventionId,
    pub role_id: RoleId,
    pub kind: CeremonyInterventionKind,
    pub target: CeremonyInterventionTarget,
    pub content: CeremonyInterventionContent,
    pub provenance: Option<CeremonyInterventionProvenance>,
    pub intent: Option<CeremonyInterventionIntent>,
    pub delivery: Option<InterventionDeliveryPolicy>,
    pub supervisor: Option<SupervisorPrincipal>,
    pub now: OffsetDateTime,
}
