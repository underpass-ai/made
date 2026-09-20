use std::collections::BTreeSet;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::DomainError;
use crate::value_objects::{CeremonyAgentExecutionId, HostAgentIncarnation};

use super::{DeliveryRecipient, InterventionRoleIds, RoleId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CeremonyInterventionTarget {
    Table,
    Roles(InterventionRoleIds),
    /// One named agent execution of one named process generation.
    ///
    /// The exact shape, and the only one that can be wrong about who
    /// answered: a seat is filled by whoever holds it, but a process
    /// generation is a particular run, and an intervention addressed to
    /// it dies with it unless the asker said to follow.
    AgentExecution(DeliveryRecipient),
}

impl CeremonyInterventionTarget {
    #[must_use]
    pub const fn table() -> Self {
        Self::Table
    }

    pub fn roles(role_ids: impl IntoIterator<Item = RoleId>) -> Result<Self, DomainError> {
        InterventionRoleIds::new(role_ids).map(Self::Roles)
    }

    #[must_use]
    pub const fn agent_execution(recipient: DeliveryRecipient) -> Self {
        Self::AgentExecution(recipient)
    }

    pub(crate) fn validate(&self) -> Result<(), DomainError> {
        match self {
            Self::Table | Self::AgentExecution(_) => Ok(()),
            Self::Roles(role_ids) => role_ids.validate(),
        }
    }

    #[must_use]
    pub fn accepts(&self, role_id: &RoleId) -> bool {
        match self {
            Self::Table => true,
            Self::Roles(role_ids) => role_ids.as_set().contains(role_id),
            Self::AgentExecution(recipient) => recipient.role_id() == role_id,
        }
    }

    #[must_use]
    pub fn role_ids(&self) -> Option<&BTreeSet<RoleId>> {
        match self {
            Self::Table | Self::AgentExecution(_) => None,
            Self::Roles(role_ids) => Some(role_ids.as_set()),
        }
    }

    #[must_use]
    pub const fn is_table(&self) -> bool {
        matches!(self, Self::Table)
    }

    /// The one recipient this target names, when it names exactly one.
    #[must_use]
    pub const fn exact_recipient(&self) -> Option<&DeliveryRecipient> {
        match self {
            Self::AgentExecution(recipient) => Some(recipient),
            Self::Table | Self::Roles(_) => None,
        }
    }

    /// Whether a recipient offering an answer is the one addressed.
    ///
    /// An exact target checks the process generation as well as the
    /// execution, because a replaced agent answering for its
    /// predecessor is precisely the confusion this target exists to
    /// prevent. A seat or the table checks only that the role is one
    /// the item was put to.
    #[must_use]
    pub fn admits_recipient(&self, recipient: &DeliveryRecipient) -> bool {
        match self {
            Self::AgentExecution(addressed) => addressed.is_same_incarnation_as(recipient),
            Self::Table | Self::Roles(_) => self.accepts(recipient.role_id()),
        }
    }

    /// The wire tag, which is also the discriminator stored in a stream.
    #[must_use]
    pub const fn kind_str(&self) -> &'static str {
        match self {
            Self::Table => "table",
            Self::Roles(_) => "roles",
            Self::AgentExecution(_) => "agent_execution",
        }
    }
}

// Hand-written rather than derived, because the two shapes that already
// exist in sealed streams have to re-serialize byte for byte and the
// third one has to be readable. An adjacently tagged derive would put
// the agent's fields under the `role_ids` key, which is a lie a reader
// of the journal would have to be taught to ignore.
impl Serialize for CeremonyInterventionTarget {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct Repr<'a> {
            kind: &'static str,
            #[serde(skip_serializing_if = "Option::is_none")]
            role_ids: Option<&'a InterventionRoleIds>,
            #[serde(skip_serializing_if = "Option::is_none")]
            agent_execution_id: Option<&'a CeremonyAgentExecutionId>,
            #[serde(skip_serializing_if = "Option::is_none")]
            incarnation: Option<&'a HostAgentIncarnation>,
            #[serde(skip_serializing_if = "Option::is_none")]
            role_id: Option<&'a RoleId>,
        }

        let recipient = self.exact_recipient();
        Repr {
            kind: self.kind_str(),
            role_ids: match self {
                Self::Roles(role_ids) => Some(role_ids),
                Self::Table | Self::AgentExecution(_) => None,
            },
            agent_execution_id: recipient.map(DeliveryRecipient::agent_execution_id),
            incarnation: recipient.map(DeliveryRecipient::incarnation),
            role_id: recipient.map(DeliveryRecipient::role_id),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CeremonyInterventionTarget {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "snake_case")]
        enum Kind {
            Table,
            Roles,
            AgentExecution,
        }

        #[derive(Deserialize)]
        struct Repr {
            kind: Kind,
            #[serde(default)]
            role_ids: Option<InterventionRoleIds>,
            #[serde(flatten, default)]
            recipient: Option<DeliveryRecipient>,
        }

        let repr = Repr::deserialize(deserializer)?;
        match repr.kind {
            Kind::Table => Ok(Self::Table),
            Kind::Roles => repr
                .role_ids
                .map(Self::Roles)
                .ok_or_else(|| serde::de::Error::missing_field("role_ids")),
            Kind::AgentExecution => repr
                .recipient
                .map(Self::AgentExecution)
                .ok_or_else(|| serde::de::Error::missing_field("agent_execution_id")),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn recipient(execution: &str, incarnation: &str) -> DeliveryRecipient {
        DeliveryRecipient::new(
            CeremonyAgentExecutionId::new(execution).unwrap(),
            HostAgentIncarnation::new(incarnation).unwrap(),
            RoleId::new("ENGINEER").unwrap(),
        )
    }

    #[test]
    fn table_accepts_every_role_and_scoped_target_does_not() {
        let engineer = RoleId::new("ENGINEER").unwrap();
        let observer = RoleId::new("OBSERVER").unwrap();

        assert!(CeremonyInterventionTarget::table().accepts(&engineer));
        let target = CeremonyInterventionTarget::roles([observer.clone()]).unwrap();
        assert!(target.accepts(&observer));
        assert!(!target.accepts(&engineer));
    }

    #[test]
    fn the_shapes_already_in_sealed_streams_keep_their_bytes() {
        assert_eq!(
            serde_json::to_value(CeremonyInterventionTarget::table()).unwrap(),
            json!({ "kind": "table" })
        );
        let roles = CeremonyInterventionTarget::roles([RoleId::new("reviewer").unwrap()]).unwrap();
        assert_eq!(
            serde_json::to_string(&roles).unwrap(),
            r#"{"kind":"roles","role_ids":["reviewer"]}"#
        );
        assert_eq!(
            serde_json::from_str::<CeremonyInterventionTarget>(
                r#"{"kind":"roles","role_ids":["reviewer"]}"#
            )
            .unwrap(),
            roles
        );
    }

    #[test]
    fn the_exact_shape_writes_its_recipient_flat() {
        let target = CeremonyInterventionTarget::agent_execution(recipient("exec-1", "inc-1"));
        let encoded = serde_json::to_value(&target).unwrap();
        assert_eq!(
            encoded,
            json!({
                "kind": "agent_execution",
                "agent_execution_id": "exec-1",
                "incarnation": "inc-1",
                "role_id": "ENGINEER"
            })
        );
        assert_eq!(
            serde_json::from_value::<CeremonyInterventionTarget>(encoded).unwrap(),
            target
        );
    }

    #[test]
    fn an_exact_target_refuses_a_replacement_process() {
        let target = CeremonyInterventionTarget::agent_execution(recipient("exec-1", "inc-1"));
        assert!(target.admits_recipient(&recipient("exec-1", "inc-1")));
        assert!(!target.admits_recipient(&recipient("exec-1", "inc-2")));
        // The seat still matches, which is exactly why the role alone
        // is not enough to decide who answered.
        assert!(target.accepts(&RoleId::new("ENGINEER").unwrap()));
    }

    #[test]
    fn a_role_target_admits_whoever_holds_the_seat() {
        let target = CeremonyInterventionTarget::roles([RoleId::new("ENGINEER").unwrap()]).unwrap();
        assert!(target.admits_recipient(&recipient("exec-9", "inc-9")));
        assert!(target.exact_recipient().is_none());
    }
}
