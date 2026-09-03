use serde::{Deserialize, Serialize};

/// One answer given at the table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterventionResponseView {
    pub role_id: String,
    pub content: String,
    pub responded_at_millis: i64,
}
