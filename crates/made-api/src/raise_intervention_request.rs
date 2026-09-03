use serde::{Deserialize, Serialize};

/// Request to put a question, investigation or proposed action to the table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaiseInterventionRequest {
    pub ceremony_id: String,
    pub intervention_id: String,
    pub role_id: String,
    pub role_kind: String,
    pub kind: String,
    pub target_role_ids: Vec<String>,
    pub request: String,
}
