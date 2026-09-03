use serde::Serialize;

use super::RetryPolicyDocument;

#[derive(Debug, Serialize)]
pub(super) struct RetryPoliciesDocument {
    pub(super) default: RetryPolicyDocument,
}
