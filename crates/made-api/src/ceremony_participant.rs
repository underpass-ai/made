use serde::{Deserialize, Serialize};

/// One seat at a ceremony, as a consumer sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyParticipant {
    pub role_id: String,
    pub specialty: String,
    pub bound_at_millis: i64,
}
