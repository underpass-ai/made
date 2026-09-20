use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::value_objects::HostKind;

use super::{Capability, IndependenceGroup};

/// What a logical participant needs from whatever plays it.
///
/// Deliberately a policy and not a selection: the design says what
/// would do, and the host says what there is. Naming a concrete agent
/// here would make the design a deployment.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParticipantBindingPolicy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    host_kind: Option<HostKind>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    capabilities: BTreeSet<Capability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    independence_group: Option<IndependenceGroup>,
}

impl ParticipantBindingPolicy {
    #[must_use]
    pub fn new(
        host_kind: Option<HostKind>,
        capabilities: impl IntoIterator<Item = Capability>,
        independence_group: Option<IndependenceGroup>,
    ) -> Self {
        Self {
            host_kind,
            capabilities: capabilities.into_iter().collect(),
            independence_group,
        }
    }

    #[must_use]
    pub const fn host_kind(&self) -> Option<&HostKind> {
        self.host_kind.as_ref()
    }

    #[must_use]
    pub const fn capabilities(&self) -> &BTreeSet<Capability> {
        &self.capabilities
    }

    #[must_use]
    pub const fn independence_group(&self) -> Option<&IndependenceGroup> {
        self.independence_group.as_ref()
    }

    /// Whether whoever plays this participant would supply that.
    #[must_use]
    pub fn supplies(&self, capability: &Capability) -> bool {
        self.capabilities.contains(capability)
    }
}
