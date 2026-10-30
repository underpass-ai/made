use time::OffsetDateTime;

use crate::entities::ProtocolEvidencePack;
use crate::value_objects::{ProtocolInterventionId, RoleId};

/// Answer an open intervention out of a configured source.
///
/// The pack names the source it came from, so the receipt that a
/// source was consulted needs nothing the pack does not carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RespondToInterventionWithEvidence {
    pub intervention_id: ProtocolInterventionId,
    pub role_id: RoleId,
    pub evidence_pack: ProtocolEvidencePack,
    pub now: OffsetDateTime,
}
