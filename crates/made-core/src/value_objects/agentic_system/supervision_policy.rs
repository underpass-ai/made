use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::value_objects::RoleAction;

use super::{GuardRef, IndependenceRule, SystemRoleId};

/// What the system insists on beyond the ceremonies themselves.
///
/// Three separate things that are easy to confuse: which guards a
/// human must answer, which roles have to be independent of which, and
/// what each role is allowed to do. A design that left any of them
/// implicit would have them decided, differently, by every run.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupervisionPolicy {
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    human_approvals: BTreeSet<GuardRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    independence: Vec<IndependenceRule>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    role_actions: BTreeMap<SystemRoleId, BTreeSet<RoleAction>>,
}

impl SupervisionPolicy {
    #[must_use]
    pub fn new(
        human_approvals: impl IntoIterator<Item = GuardRef>,
        independence: impl IntoIterator<Item = IndependenceRule>,
        role_actions: impl IntoIterator<Item = (SystemRoleId, BTreeSet<RoleAction>)>,
    ) -> Self {
        Self {
            human_approvals: human_approvals.into_iter().collect(),
            independence: independence.into_iter().collect(),
            role_actions: role_actions.into_iter().collect(),
        }
    }

    #[must_use]
    pub const fn human_approvals(&self) -> &BTreeSet<GuardRef> {
        &self.human_approvals
    }

    #[must_use]
    pub fn independence(&self) -> &[IndependenceRule] {
        &self.independence
    }

    #[must_use]
    pub const fn role_actions(&self) -> &BTreeMap<SystemRoleId, BTreeSet<RoleAction>> {
        &self.role_actions
    }

    /// Whether anything at all is supervised.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.human_approvals.is_empty()
            && self.independence.is_empty()
            && self.role_actions.is_empty()
    }
}
