use serde::{Deserialize, Serialize};

use crate::value_objects::{CeremonyInterventionId, CeremonyInterventionResponse};

/// A seat answered an intervention.
///
/// The response as the aggregate appended it, evidence pack included
/// when the answer came out of a configured source. The `answers`
/// reason the aggregate records beside every response is its own
/// inference from this event and is not repeated here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterventionResponded {
    pub intervention_id: CeremonyInterventionId,
    pub response: CeremonyInterventionResponse,
}
