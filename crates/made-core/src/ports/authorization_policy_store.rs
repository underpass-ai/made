use async_trait::async_trait;

use crate::entities::AuthorizationPolicyEvent;
use crate::value_objects::{
    AuthorizationDecisionId, AuthorizationDecisionPageLimit, AuthorizationPolicyId,
    AuthorizationPolicyVersion,
};
use crate::DomainError;

use super::{
    AuthorizationDecisionPage, AuthorizationPolicyAppendOutcome, AuthorizationPolicySnapshot,
};

#[async_trait]
pub trait AuthorizationPolicyStorePort: Send + Sync {
    async fn load(
        &self,
        policy_id: &AuthorizationPolicyId,
    ) -> Result<Option<AuthorizationPolicySnapshot>, DomainError>;

    async fn append(
        &self,
        policy_id: &AuthorizationPolicyId,
        expected: AuthorizationPolicyVersion,
        events: Vec<AuthorizationPolicyEvent>,
    ) -> Result<AuthorizationPolicyAppendOutcome, DomainError>;

    async fn decisions(
        &self,
        policy_id: &AuthorizationPolicyId,
        after: Option<&AuthorizationDecisionId>,
        limit: AuthorizationDecisionPageLimit,
    ) -> Result<AuthorizationDecisionPage, DomainError>;
}
