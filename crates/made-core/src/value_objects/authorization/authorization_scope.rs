use serde::{Deserialize, Serialize};

use crate::value_objects::{
    ArtifactId, BudgetAccountId, CouncilId, ProtocolId, ProtocolName, ProtocolVersion,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum AuthorizationScope {
    Global,
    Protocol {
        protocol_id: ProtocolId,
    },
    ProtocolTree {
        root_id: ProtocolId,
    },
    /// Authoritative lineage resolved by the application before policy evaluation.
    /// This scope is request-only and cannot be issued as a grant.
    ResolvedProtocol {
        protocol_id: ProtocolId,
        root_id: ProtocolId,
    },
    Definition {
        name: ProtocolName,
        version: Option<ProtocolVersion>,
    },
    Artifact {
        artifact_id: ArtifactId,
    },
    Council {
        council_id: CouncilId,
    },
    Budget {
        account_id: BudgetAccountId,
    },
}

impl AuthorizationScope {
    #[must_use]
    pub fn covers(&self, requested: &Self) -> bool {
        match (self, requested) {
            (Self::Global, _) => true,
            (Self::Protocol { protocol_id: left }, Self::Protocol { protocol_id: right })
            | (Self::ProtocolTree { root_id: left }, Self::ProtocolTree { root_id: right }) => {
                left == right
            }
            (Self::ProtocolTree { root_id }, Self::Protocol { protocol_id }) => {
                root_id == protocol_id
            }
            (
                Self::Protocol {
                    protocol_id: granted,
                },
                Self::ResolvedProtocol { protocol_id, .. },
            ) => granted == protocol_id,
            (Self::ProtocolTree { root_id: granted }, Self::ResolvedProtocol { root_id, .. }) => {
                granted == root_id
            }
            (
                Self::Definition {
                    name: left_name,
                    version: left_version,
                },
                Self::Definition {
                    name: right_name,
                    version: right_version,
                },
            ) => {
                left_name == right_name && (left_version.is_none() || left_version == right_version)
            }
            (Self::Artifact { artifact_id: left }, Self::Artifact { artifact_id: right }) => {
                left == right
            }
            (Self::Council { council_id: left }, Self::Council { council_id: right }) => {
                left == right
            }
            (Self::Budget { account_id: left }, Self::Budget { account_id: right }) => {
                left == right
            }
            _ => false,
        }
    }

    #[must_use]
    pub const fn is_resolved_request_scope(&self) -> bool {
        matches!(self, Self::ResolvedProtocol { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_and_tree_grants_cover_the_same_resolved_child() {
        let child = ProtocolId::new("child").unwrap();
        let root = ProtocolId::new("root").unwrap();
        let resolved = AuthorizationScope::ResolvedProtocol {
            protocol_id: child.clone(),
            root_id: root.clone(),
        };

        assert!(AuthorizationScope::Protocol { protocol_id: child }.covers(&resolved));
        assert!(AuthorizationScope::ProtocolTree { root_id: root }.covers(&resolved));
    }

    #[test]
    fn resolved_child_rejects_neighbor_grants() {
        let resolved = AuthorizationScope::ResolvedProtocol {
            protocol_id: ProtocolId::new("child").unwrap(),
            root_id: ProtocolId::new("root").unwrap(),
        };

        assert!(!AuthorizationScope::Protocol {
            protocol_id: ProtocolId::new("neighbor").unwrap()
        }
        .covers(&resolved));
        assert!(!AuthorizationScope::ProtocolTree {
            root_id: ProtocolId::new("other-root").unwrap()
        }
        .covers(&resolved));
    }
}
