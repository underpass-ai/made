use time::OffsetDateTime;

use crate::entities::CeremonyEvidencePack;
use crate::value_objects::{CeremonyInterventionId, RoleId};

/// Answer an open intervention out of a configured source.
///
/// The pack names the source it came from, so the receipt that a
/// source was consulted needs nothing the pack does not carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RespondToInterventionWithEvidence {
    pub intervention_id: CeremonyInterventionId,
    pub role_id: RoleId,
    pub evidence_pack: CeremonyEvidencePack,
    pub now: OffsetDateTime,
}
