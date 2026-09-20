use serde::{Deserialize, Serialize};

use crate::value_objects::{CeremonyAgentExecutionId, HostAgentIncarnation, HostDeliveryTarget};

use super::RoleId;

/// The one live agent an intervention was put in front of.
///
/// Three parts because all three are needed to say "this one and not
/// that one": the execution names the piece of work, the incarnation
/// names the process generation doing it, and the role names the seat
/// it holds. Drop the incarnation and a replaced agent answers for its
/// predecessor; drop the role and the answer cannot be checked against
/// the item's target.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DeliveryRecipient {
    agent_execution_id: CeremonyAgentExecutionId,
    incarnation: HostAgentIncarnation,
    role_id: RoleId,
}

impl DeliveryRecipient {
    #[must_use]
    pub const fn new(
        agent_execution_id: CeremonyAgentExecutionId,
        incarnation: HostAgentIncarnation,
        role_id: RoleId,
    ) -> Self {
        Self {
            agent_execution_id,
            incarnation,
            role_id,
        }
    }

    #[must_use]
    pub const fn agent_execution_id(&self) -> &CeremonyAgentExecutionId {
        &self.agent_execution_id
    }

    #[must_use]
    pub const fn incarnation(&self) -> &HostAgentIncarnation {
        &self.incarnation
    }

    #[must_use]
    pub const fn role_id(&self) -> &RoleId {
        &self.role_id
    }

    /// The ledger destination that names exactly this recipient.
    #[must_use]
    pub fn exact_target(&self) -> HostDeliveryTarget {
        HostDeliveryTarget::agent_execution(
            self.agent_execution_id.clone(),
            self.incarnation.clone(),
        )
    }

    /// The ledger destination for the seat, whoever is sitting in it.
    #[must_use]
    pub fn role_target(&self) -> HostDeliveryTarget {
        HostDeliveryTarget::role(self.role_id.clone())
    }

    /// Whether this is the same execution *and* the same generation.
    ///
    /// The comparison an exact target is checked with: a new process
    /// picking up the same execution is a different recipient, and
    /// letting it acknowledge would credit one agent with another's
    /// answer.
    #[must_use]
    pub fn is_same_incarnation_as(&self, other: &Self) -> bool {
        self.agent_execution_id == other.agent_execution_id && self.incarnation == other.incarnation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recipient(execution: &str, incarnation: &str) -> DeliveryRecipient {
        DeliveryRecipient::new(
            CeremonyAgentExecutionId::new(execution).unwrap(),
            HostAgentIncarnation::new(incarnation).unwrap(),
            RoleId::new("ENGINEER").unwrap(),
        )
    }

    #[test]
    fn a_replacement_process_is_not_the_same_recipient() {
        let first = recipient("exec-1", "inc-1");
        assert!(first.is_same_incarnation_as(&recipient("exec-1", "inc-1")));
        assert!(!first.is_same_incarnation_as(&recipient("exec-1", "inc-2")));
        assert!(!first.is_same_incarnation_as(&recipient("exec-2", "inc-1")));
    }

    #[test]
    fn it_names_both_the_process_and_the_seat_as_destinations() {
        let recipient = recipient("exec-1", "inc-1");
        assert_eq!(
            recipient.exact_target().target_key().as_str(),
            "agent:exec-1:inc-1"
        );
        assert_eq!(
            recipient.role_target().target_key().as_str(),
            "role:ENGINEER"
        );
    }
}
