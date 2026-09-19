//! A ceremony fact paired with the authorization decision that admitted it.

use crate::entities::AuditFact;
use crate::value_objects::AuthorizationEvidence;

/// Input that can only be sealed as an authorization-bearing audit record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizedAuditFact {
    fact: AuditFact,
    evidence: AuthorizationEvidence,
}

impl AuthorizedAuditFact {
    pub(crate) fn new(fact: AuditFact, evidence: AuthorizationEvidence) -> Self {
        Self { fact, evidence }
    }

    pub(crate) fn into_parts(self) -> (AuditFact, AuthorizationEvidence) {
        (self.fact, self.evidence)
    }
}
