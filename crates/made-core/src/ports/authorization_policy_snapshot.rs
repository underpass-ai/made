use crate::entities::AuthorizationPolicy;
use crate::value_objects::AuthorizationPolicyVersion;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationPolicySnapshot {
    pub version: AuthorizationPolicyVersion,
    pub policy: AuthorizationPolicy,
}
