use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::entities::CeremonyEvidencePack;
use crate::value_objects::{CeremonyEvidenceSourceId, CeremonyInterventionId, RoleId};

/// A configured source was consulted to answer an intervention.
///
/// Carries the pack that was fetched. The same pack also travels inside
/// the [`super::InterventionResponded`] event sealed with it, because
/// that is the event a fold applies; this one is the receipt that a
/// source was looked into, complete enough to be read on its own.
/// `collected_at` is when the ceremony took the evidence in; the pack's
/// own `collected_at` is when the source produced it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceCollected {
    pub intervention_id: CeremonyInterventionId,
    pub source_id: CeremonyEvidenceSourceId,
    pub collected_by: RoleId,
    pub evidence_pack: CeremonyEvidencePack,
    #[serde(with = "time::serde::rfc3339")]
    pub collected_at: OffsetDateTime,
}
