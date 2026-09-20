use made_app::usecases::RespondToCeremonyInterventionInput;
use made_core::value_objects::{
    AuditActorKind, CeremonyAgentExecutionId, CeremonyId, CeremonyInterventionContent,
    CeremonyInterventionId, DeliveryRecipient, HostAgentIncarnation, HostDeliveryId, RoleId,
};
use made_embedded::EmbeddedMade;
use serde_json::Value;

use super::embedded_request_fields::{
    optional_attributes, optional_string, required_actor_kind, required_string,
};

use crate::protocol::ToolError;

/// Validated MCP request that records one role's intervention response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EmbeddedRespondToCeremonyInterventionRequest {
    ceremony_id: CeremonyId,
    intervention_id: CeremonyInterventionId,
    role_id: RoleId,
    role_kind: AuditActorKind,
    content: CeremonyInterventionContent,
    answered_delivery: Option<(DeliveryRecipient, HostDeliveryId)>,
}

impl EmbeddedRespondToCeremonyInterventionRequest {
    pub(super) async fn execute(self, made: &EmbeddedMade) -> Result<CeremonyId, ToolError> {
        let mut input = RespondToCeremonyInterventionInput::new(
            self.ceremony_id.clone(),
            self.intervention_id,
            self.role_id,
            self.role_kind,
            self.content,
        );
        if let Some((recipient, delivery_id)) = self.answered_delivery {
            input = input.answering_delivery(recipient, delivery_id);
        }
        made.respond_to_intervention(input).await?;
        Ok(self.ceremony_id)
    }
}

impl TryFrom<&Value> for EmbeddedRespondToCeremonyInterventionRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let object = value
            .as_object()
            .ok_or_else(|| "tools/call.arguments must be an object".to_owned())?;
        let role =
            RoleId::new(required_string(object, "role_id")?).map_err(|error| error.to_string())?;
        Ok(Self {
            ceremony_id: CeremonyId::new(required_string(object, "ceremony_id")?)
                .map_err(|error| error.to_string())?,
            intervention_id: CeremonyInterventionId::new(required_string(
                object,
                "intervention_id",
            )?)
            .map_err(|error| error.to_string())?,
            role_kind: required_actor_kind(object, "role_kind")?,
            role_id: role.clone(),
            content: CeremonyInterventionContent::new(
                required_string(object, "message")?,
                optional_attributes(object, "details")?,
            )
            .map_err(|error| error.to_string())?,
            answered_delivery: answered_delivery(object, &role)?,
        })
    }
}

/// All three or none.
///
/// An answer that names its offer but not its agent cannot be checked
/// against the item's target, and one that names its agent but not the
/// offer cannot be checked against the ledger; either half alone is an
/// unverifiable claim about who answered.
fn answered_delivery(
    object: &serde_json::Map<String, Value>,
    role: &RoleId,
) -> Result<Option<(DeliveryRecipient, HostDeliveryId)>, String> {
    match (
        optional_string(object, "delivery_id")?,
        optional_string(object, "agent_execution_id")?,
        optional_string(object, "incarnation")?,
    ) {
        (Some(delivery_id), Some(execution), Some(incarnation)) => Ok(Some((
            DeliveryRecipient::new(
                CeremonyAgentExecutionId::new(execution).map_err(|error| error.to_string())?,
                HostAgentIncarnation::new(incarnation).map_err(|error| error.to_string())?,
                role.clone(),
            ),
            HostDeliveryId::new(delivery_id).map_err(|error| error.to_string())?,
        ))),
        (None, None, None) => Ok(None),
        _ => Err(
            "answering a delivery needs `delivery_id`, `agent_execution_id` and `incarnation`"
                .to_owned(),
        ),
    }
}
