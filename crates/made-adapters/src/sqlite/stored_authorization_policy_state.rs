use made_core::entities::AuthorizationPolicyEvent;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(in crate::sqlite) struct StoredAuthorizationPolicyState {
    pub(in crate::sqlite) events: Vec<AuthorizationPolicyEvent>,
}
