use time::OffsetDateTime;

use crate::value_objects::{CeremonyInterventionId, RoleId};

/// Judge an intervention answered enough; only its requester may.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloseIntervention {
    pub intervention_id: CeremonyInterventionId,
    pub role_id: RoleId,
    pub now: OffsetDateTime,
}
