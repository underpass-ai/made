use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::value_objects::{
    AuthenticatedPrincipal, AuthorizationDecision, AuthorizationGrant, AuthorizationPolicyId,
    AuthorizationRevocation, SeparationRule,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum AuthorizationPolicyEvent {
    Opened {
        policy_id: AuthorizationPolicyId,
        owner: AuthenticatedPrincipal,
        separation_rules: Vec<SeparationRule>,
        #[serde(with = "time::serde::rfc3339")]
        opened_at: OffsetDateTime,
    },
    GrantIssued {
        policy_id: AuthorizationPolicyId,
        grant: AuthorizationGrant,
        #[serde(with = "time::serde::rfc3339")]
        issued_at: OffsetDateTime,
    },
    GrantRevoked {
        policy_id: AuthorizationPolicyId,
        revocation: AuthorizationRevocation,
    },
    DecisionRecorded {
        policy_id: AuthorizationPolicyId,
        decision: AuthorizationDecision,
    },
}

impl AuthorizationPolicyEvent {
    #[must_use]
    pub const fn policy_id(&self) -> &AuthorizationPolicyId {
        match self {
            Self::Opened { policy_id, .. }
            | Self::GrantIssued { policy_id, .. }
            | Self::GrantRevoked { policy_id, .. }
            | Self::DecisionRecorded { policy_id, .. } => policy_id,
        }
    }
}
