use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::entities::CeremonyEvidencePack;
use crate::error::DomainError;

use crate::value_objects::HostDeliveryId;

use super::{CeremonyInterventionContent, DeliveryRecipient, RoleId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyInterventionResponse {
    role_id: RoleId,
    content: CeremonyInterventionContent,
    #[serde(default)]
    evidence_pack: Option<CeremonyEvidencePack>,
    #[serde(with = "time::serde::rfc3339")]
    responded_at: OffsetDateTime,
    // Provenance, not lease bookkeeping: which live agent answered and
    // which offer of the item it was answering. Absent on every
    // response sealed before routing existed, and skipped when absent
    // so those bytes do not move.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    executor: Option<DeliveryRecipient>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    delivery_id: Option<HostDeliveryId>,
}

impl CeremonyInterventionResponse {
    #[must_use]
    pub fn new(
        role_id: RoleId,
        content: CeremonyInterventionContent,
        responded_at: OffsetDateTime,
    ) -> Self {
        Self {
            role_id,
            content,
            evidence_pack: None,
            responded_at,
            executor: None,
            delivery_id: None,
        }
    }

    /// The same answer, attributed to the agent that was handed the item.
    #[must_use]
    pub fn answering_delivery(
        mut self,
        executor: DeliveryRecipient,
        delivery_id: HostDeliveryId,
    ) -> Self {
        self.executor = Some(executor);
        self.delivery_id = Some(delivery_id);
        self
    }

    pub fn from_evidence(
        role_id: RoleId,
        evidence_pack: CeremonyEvidencePack,
        responded_at: OffsetDateTime,
    ) -> Result<Self, DomainError> {
        let content = evidence_pack.intervention_content()?;
        Ok(Self {
            role_id,
            content,
            evidence_pack: Some(evidence_pack),
            responded_at,
            executor: None,
            delivery_id: None,
        })
    }

    #[must_use]
    pub fn role_id(&self) -> &RoleId {
        &self.role_id
    }

    #[must_use]
    pub fn content(&self) -> &CeremonyInterventionContent {
        &self.content
    }

    #[must_use]
    pub fn evidence_pack(&self) -> Option<&CeremonyEvidencePack> {
        self.evidence_pack.as_ref()
    }

    #[must_use]
    pub fn responded_at(&self) -> OffsetDateTime {
        self.responded_at
    }

    /// The live agent that gave this answer, when one was named.
    #[must_use]
    pub const fn executor(&self) -> Option<&DeliveryRecipient> {
        self.executor.as_ref()
    }

    /// The offer of the item this answer closes, when one was named.
    #[must_use]
    pub const fn delivery_id(&self) -> Option<&HostDeliveryId> {
        self.delivery_id.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::*;
    use crate::value_objects::{
        Attributes, CeremonyAgentExecutionId, CeremonyInterventionContent, HostAgentIncarnation,
    };

    #[test]
    fn an_answer_with_no_executor_keeps_the_bytes_it_always_had() {
        let response = CeremonyInterventionResponse::new(
            RoleId::new("reviewer").unwrap(),
            CeremonyInterventionContent::new("Yes.", Attributes::empty()).unwrap(),
            datetime!(2026-07-29 09:05:00 UTC),
        );
        // Streamed rather than mapped, because the map a `Value`
        // carries sorts its keys and would hide a field that moved.
        assert_eq!(
            serde_json::to_string(&response).unwrap(),
            r#"{"role_id":"reviewer","content":{"message":"Yes.","details":{}},"evidence_pack":null,"responded_at":"2026-07-29T09:05:00Z"}"#
        );
    }

    #[test]
    fn an_attributed_answer_names_the_agent_and_the_offer() {
        let response = CeremonyInterventionResponse::new(
            RoleId::new("reviewer").unwrap(),
            CeremonyInterventionContent::new("Yes.", Attributes::empty()).unwrap(),
            datetime!(2026-07-29 09:05:00 UTC),
        )
        .answering_delivery(
            DeliveryRecipient::new(
                CeremonyAgentExecutionId::new("exec-1").unwrap(),
                HostAgentIncarnation::new("inc-1").unwrap(),
                RoleId::new("reviewer").unwrap(),
            ),
            HostDeliveryId::new("c-1:intervention:i-1:agent:exec-1:inc-1").unwrap(),
        );
        assert_eq!(response.executor().unwrap().incarnation().as_str(), "inc-1");
        assert!(response.delivery_id().is_some());
    }
}
