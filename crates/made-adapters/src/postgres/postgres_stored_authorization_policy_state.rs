use made_core::entities::AuthorizationPolicyEvent;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct PostgresStoredAuthorizationPolicyState {
    pub(super) events: Vec<AuthorizationPolicyEvent>,
}
