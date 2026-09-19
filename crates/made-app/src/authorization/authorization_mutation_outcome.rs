use made_core::value_objects::AuthorizationPolicyVersion;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorizationMutationOutcome {
    Applied { version: AuthorizationPolicyVersion },
    Existing { version: AuthorizationPolicyVersion },
}
