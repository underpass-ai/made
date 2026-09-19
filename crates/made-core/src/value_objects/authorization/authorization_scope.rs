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
}
