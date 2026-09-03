use serde::{Deserialize, Serialize};

/// Request to answer an open intervention.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RespondToInterventionRequest {
    pub ceremony_id: String,
    pub intervention_id: String,
    pub role_id: String,
    pub role_kind: String,
    pub content: String,
}
