use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::value_objects::{ProtocolInterventionId, RoleId};

/// The requesting seat judged an intervention answered enough.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterventionClosed {
    pub intervention_id: ProtocolInterventionId,
    pub closed_by: RoleId,
    #[serde(with = "time::serde::rfc3339")]
    pub closed_at: OffsetDateTime,
}
