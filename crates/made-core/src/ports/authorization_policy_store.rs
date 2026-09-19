use async_trait::async_trait;

use crate::entities::AuthorizationPolicyEvent;
use crate::value_objects::{
    AuthorizationDecision, AuthorizationDecisionId, AuthorizationDecisionPageLimit,
    AuthorizationPolicyId, AuthorizationPolicyVersion, AuthorizationRequestId,
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

    async fn decision(
        &self,
        policy_id: &AuthorizationPolicyId,
        decision_id: &AuthorizationDecisionId,
    ) -> Result<Option<AuthorizationDecision>, DomainError>;

    async fn decision_for_request(
        &self,
        policy_id: &AuthorizationPolicyId,
        request_id: &AuthorizationRequestId,
    ) -> Result<Option<AuthorizationDecision>, DomainError>;
}
