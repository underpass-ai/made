use std::sync::Arc;

use made_core::ports::{AuthorizationDecisionPage, AuthorizationPolicyStorePort};
use made_core::value_objects::{
    AuthorizationDecisionId, AuthorizationDecisionPageLimit, AuthorizationPolicyId,
};
use made_core::DomainError;

#[derive(Clone)]
pub struct ReadAuthorizationDecisionsUseCase {
    policy_id: AuthorizationPolicyId,
    store: Arc<dyn AuthorizationPolicyStorePort>,
}

impl std::fmt::Debug for ReadAuthorizationDecisionsUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ReadAuthorizationDecisionsUseCase")
            .field("policy_id", &self.policy_id)
            .finish_non_exhaustive()
    }
}

impl ReadAuthorizationDecisionsUseCase {
    #[must_use]
    pub fn new(
        policy_id: AuthorizationPolicyId,
        store: Arc<dyn AuthorizationPolicyStorePort>,
    ) -> Self {
        Self { policy_id, store }
    }

    pub async fn execute(
        &self,
        after: Option<&AuthorizationDecisionId>,
        limit: AuthorizationDecisionPageLimit,
    ) -> Result<AuthorizationDecisionPage, DomainError> {
        self.store.decisions(&self.policy_id, after, limit).await
    }
}
