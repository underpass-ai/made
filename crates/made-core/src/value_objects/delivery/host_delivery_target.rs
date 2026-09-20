use serde::{Deserialize, Serialize};

use crate::value_objects::{CeremonyAgentExecutionId, HostAgentIncarnation, RoleId};

use super::{HostDeliveryTargetKey, IntegratorBindingId};

/// Where one delivery is addressed.
///
/// Three shapes because a host is reachable three ways and the ledger
/// must tell them apart: a named process generation, a seat at the
/// table nobody has claimed yet, and a bound integrator.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HostDeliveryTarget {
    /// One live agent execution of one host process generation.
    AgentExecution {
        agent_execution_id: CeremonyAgentExecutionId,
        incarnation: HostAgentIncarnation,
    },
    /// Any host that can act for a role, for a destination that pulls.
    Role { role_id: RoleId },
    /// The integrator bound to a ceremony or a system execution.
    IntegratorBinding { binding_id: IntegratorBindingId },
}

impl HostDeliveryTarget {
    #[must_use]
    pub const fn agent_execution(
        agent_execution_id: CeremonyAgentExecutionId,
        incarnation: HostAgentIncarnation,
    ) -> Self {
        Self::AgentExecution {
            agent_execution_id,
            incarnation,
        }
    }

    #[must_use]
    pub const fn role(role_id: RoleId) -> Self {
        Self::Role { role_id }
    }

    #[must_use]
    pub const fn integrator_binding(binding_id: IntegratorBindingId) -> Self {
        Self::IntegratorBinding { binding_id }
    }

    /// The stable spelling this destination is stored and scanned by.
    #[must_use]
    pub fn target_key(&self) -> HostDeliveryTargetKey {
        HostDeliveryTargetKey::new(match self {
            Self::AgentExecution {
                agent_execution_id,
                incarnation,
            } => format!("agent:{agent_execution_id}:{incarnation}"),
            Self::Role { role_id } => format!("role:{role_id}"),
            Self::IntegratorBinding { binding_id } => format!("binding:{binding_id}"),
        })
    }

    /// The role this destination acts for, when it names one.
    #[must_use]
    pub const fn role_id(&self) -> Option<&RoleId> {
        match self {
            Self::Role { role_id } => Some(role_id),
            Self::AgentExecution { .. } | Self::IntegratorBinding { .. } => None,
        }
    }

    /// The process generation this destination is fenced to, if any.
    #[must_use]
    pub const fn incarnation(&self) -> Option<&HostAgentIncarnation> {
        match self {
            Self::AgentExecution { incarnation, .. } => Some(incarnation),
            Self::Role { .. } | Self::IntegratorBinding { .. } => None,
        }
    }

    /// The binding this destination belongs to, if any.
    #[must_use]
    pub const fn binding_id(&self) -> Option<&IntegratorBindingId> {
        match self {
            Self::IntegratorBinding { binding_id } => Some(binding_id),
            Self::AgentExecution { .. } | Self::Role { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_shape_has_its_own_stable_key() {
        let agent = HostDeliveryTarget::agent_execution(
            CeremonyAgentExecutionId::new("x-1").unwrap(),
            HostAgentIncarnation::new("run-7").unwrap(),
        );
        let role = HostDeliveryTarget::role(RoleId::new("ENGINEER").unwrap());
        let binding =
            HostDeliveryTarget::integrator_binding(IntegratorBindingId::new("b-1").unwrap());

        assert_eq!(agent.target_key().as_str(), "agent:x-1:run-7");
        assert_eq!(role.target_key().as_str(), "role:ENGINEER");
        assert_eq!(binding.target_key().as_str(), "binding:b-1");
        assert_eq!(agent.target_key(), agent.target_key());
    }
}
