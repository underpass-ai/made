use time::OffsetDateTime;

use crate::value_objects::{
    CeremonyInterventionContent, CeremonyInterventionId, CeremonyInterventionKind,
    CeremonyInterventionProvenance, CeremonyInterventionTarget, RoleId,
};

/// Ask the table for something.
///
/// `provenance` is present when the item was selected out of an
/// earlier intervention's response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestIntervention {
    pub intervention_id: CeremonyInterventionId,
    pub role_id: RoleId,
    pub kind: CeremonyInterventionKind,
    pub target: CeremonyInterventionTarget,
    pub content: CeremonyInterventionContent,
    pub provenance: Option<CeremonyInterventionProvenance>,
    pub now: OffsetDateTime,
}
