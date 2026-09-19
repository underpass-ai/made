use serde::{Deserialize, Serialize};

use crate::value_objects::{
    ArtifactId, BudgetAccountId, CeremonyId, CeremonyName, CeremonyVersion, CouncilId,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum AuthorizationScope {
    Global,
    Ceremony {
        ceremony_id: CeremonyId,
    },
    CeremonyTree {
        root_id: CeremonyId,
    },
    /// Authoritative lineage resolved by the application before policy evaluation.
    /// This scope is request-only and cannot be issued as a grant.
    ResolvedCeremony {
        ceremony_id: CeremonyId,
        root_id: CeremonyId,
    },
    Definition {
        name: CeremonyName,
        version: Option<CeremonyVersion>,
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
            (Self::Ceremony { ceremony_id: left }, Self::Ceremony { ceremony_id: right })
            | (Self::CeremonyTree { root_id: left }, Self::CeremonyTree { root_id: right }) => {
                left == right
            }
            (Self::CeremonyTree { root_id }, Self::Ceremony { ceremony_id }) => {
                root_id == ceremony_id
            }
            (
                Self::Ceremony {
                    ceremony_id: granted,
                },
                Self::ResolvedCeremony { ceremony_id, .. },
            ) => granted == ceremony_id,
            (Self::CeremonyTree { root_id: granted }, Self::ResolvedCeremony { root_id, .. }) => {
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
        matches!(self, Self::ResolvedCeremony { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ceremony_and_tree_grants_cover_the_same_resolved_child() {
        let child = CeremonyId::new("child").unwrap();
        let root = CeremonyId::new("root").unwrap();
        let resolved = AuthorizationScope::ResolvedCeremony {
            ceremony_id: child.clone(),
            root_id: root.clone(),
        };

        assert!(AuthorizationScope::Ceremony { ceremony_id: child }.covers(&resolved));
        assert!(AuthorizationScope::CeremonyTree { root_id: root }.covers(&resolved));
    }

    #[test]
    fn resolved_child_rejects_neighbor_grants() {
        let resolved = AuthorizationScope::ResolvedCeremony {
            ceremony_id: CeremonyId::new("child").unwrap(),
            root_id: CeremonyId::new("root").unwrap(),
        };

        assert!(!AuthorizationScope::Ceremony {
            ceremony_id: CeremonyId::new("neighbor").unwrap()
        }
        .covers(&resolved));
        assert!(!AuthorizationScope::CeremonyTree {
            root_id: CeremonyId::new("other-root").unwrap()
        }
        .covers(&resolved));
    }
}
