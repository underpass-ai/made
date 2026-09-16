use time::OffsetDateTime;

use crate::value_objects::{CeremonyInterventionContent, CeremonyInterventionId, RoleId};

/// Answer an open intervention from a seat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RespondToIntervention {
    pub intervention_id: CeremonyInterventionId,
    pub role_id: RoleId,
    pub content: CeremonyInterventionContent,
    pub now: OffsetDateTime,
}
