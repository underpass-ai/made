use choreo_core::error::DomainError;
use choreo_core::value_objects::RetryPolicy;
use serde::Deserialize;

use super::retry_policy_document::RetryPolicyDocument;

#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct RetryPoliciesDocument {
    #[serde(default)]
    default: Option<RetryPolicyDocument>,
}

impl RetryPoliciesDocument {
    pub(super) fn default_policy(self) -> Result<RetryPolicy, DomainError> {
        self.default.map_or_else(
            || Ok(RetryPolicy::single_attempt()),
            RetryPolicyDocument::into_domain,
        )
    }
}
