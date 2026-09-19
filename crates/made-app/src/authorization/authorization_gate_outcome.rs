use made_core::value_objects::{AuthorizationDecision, AuthorizationEvidence};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorizationGateOutcome {
    Allowed {
        decision: AuthorizationDecision,
        evidence: AuthorizationEvidence,
    },
    Denied {
        decision: AuthorizationDecision,
    },
    Expired {
        decision: AuthorizationDecision,
    },
}

impl AuthorizationGateOutcome {
    #[must_use]
    pub const fn decision(&self) -> &AuthorizationDecision {
        match self {
            Self::Allowed { decision, .. }
            | Self::Denied { decision }
            | Self::Expired { decision } => decision,
        }
    }

    #[must_use]
    pub const fn evidence(&self) -> Option<&AuthorizationEvidence> {
        match self {
            Self::Allowed { evidence, .. } => Some(evidence),
            Self::Denied { .. } | Self::Expired { .. } => None,
        }
    }
}
