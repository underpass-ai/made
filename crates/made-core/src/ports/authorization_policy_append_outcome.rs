use crate::value_objects::AuthorizationPolicyVersion;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorizationPolicyAppendOutcome {
    Appended {
        version: AuthorizationPolicyVersion,
    },
    Conflict {
        expected: AuthorizationPolicyVersion,
        actual: AuthorizationPolicyVersion,
    },
}
