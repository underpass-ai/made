use std::sync::Arc;

use made_core::ports::{AuthorizationPolicySnapshot, AuthorizationPolicyStorePort};
use made_core::value_objects::AuthorizationPolicyId;
use made_core::DomainError;

#[derive(Clone)]
pub struct ReadAuthorizationPolicyUseCase {
    policy_id: AuthorizationPolicyId,
    store: Arc<dyn AuthorizationPolicyStorePort>,
}

impl std::fmt::Debug for ReadAuthorizationPolicyUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ReadAuthorizationPolicyUseCase")
            .field("policy_id", &self.policy_id)
            .finish_non_exhaustive()
    }
}

impl ReadAuthorizationPolicyUseCase {
    #[must_use]
    pub fn new(
        policy_id: AuthorizationPolicyId,
        store: Arc<dyn AuthorizationPolicyStorePort>,
    ) -> Self {
        Self { policy_id, store }
    }

    pub async fn execute(&self) -> Result<AuthorizationPolicySnapshot, DomainError> {
        self.store
            .load(&self.policy_id)
            .await?
            .ok_or(DomainError::NotFound {
                what: "authorization_policy",
            })
    }
}
