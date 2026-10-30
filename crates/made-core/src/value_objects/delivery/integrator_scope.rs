use serde::{Deserialize, Serialize};

use crate::value_objects::{AgenticSystemExecutionId, ProtocolId};

use super::IntegratorScopeKey;

/// What an integrator is bound to.
///
/// One protocol, or one execution of a composed system that owns
/// several. The second exists so an integrator driving a system is
/// woken once for the whole of it instead of once per protocol.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IntegratorScope {
    Protocol {
        protocol_id: ProtocolId,
    },
    SystemExecution {
        system_execution_id: AgenticSystemExecutionId,
    },
}

impl IntegratorScope {
    #[must_use]
    pub const fn protocol(protocol_id: ProtocolId) -> Self {
        Self::Protocol { protocol_id }
    }

    #[must_use]
    pub const fn system_execution(system_execution_id: AgenticSystemExecutionId) -> Self {
        Self::SystemExecution {
            system_execution_id,
        }
    }

    /// The stable spelling this scope is stored under.
    #[must_use]
    pub fn scope_key(&self) -> IntegratorScopeKey {
        IntegratorScopeKey::new(match self {
            Self::Protocol { protocol_id } => format!("protocol:{protocol_id}"),
            Self::SystemExecution {
                system_execution_id,
            } => format!("system_execution:{system_execution_id}"),
        })
    }

    /// The protocol this scope names, when it names one.
    #[must_use]
    pub const fn protocol_id(&self) -> Option<&ProtocolId> {
        match self {
            Self::Protocol { protocol_id } => Some(protocol_id),
            Self::SystemExecution { .. } => None,
        }
    }

    /// The system execution this scope names, when it names one.
    #[must_use]
    pub const fn system_execution_id(&self) -> Option<&AgenticSystemExecutionId> {
        match self {
            Self::SystemExecution {
                system_execution_id,
            } => Some(system_execution_id),
            Self::Protocol { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_scopes_never_share_a_key() {
        let protocol = IntegratorScope::protocol(ProtocolId::new("c-1").unwrap());
        let execution =
            IntegratorScope::system_execution(AgenticSystemExecutionId::new("c-1").unwrap());
        assert_ne!(protocol.scope_key(), execution.scope_key());
        assert_eq!(protocol.scope_key().as_str(), "protocol:c-1");
    }
}
