use made_app::budgets::StartBudgetedProtocolInput;
use made_app::usecases::StartProtocolInput;
use made_core::value_objects::{
    AuditActorKind, ProtocolContext, ProtocolId, ProtocolName, ProtocolVersion,
};
use made_embedded::EmbeddedMade;
use serde_json::Value;
use uuid::Uuid;

use super::embedded_request_fields::{
    context_from_json, optional_string, required_actor_kind, required_string,
};

use crate::mcp_protocol::ToolError;

/// Validated MCP request that starts a protocol from a published
/// definition.
///
/// It names a version instead of carrying a document. That is the whole
/// difference: the instance is bound to a definition that can be looked
/// up and checked afterwards, rather than to one that existed only in
/// the call that started it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EmbeddedStartPublishedProtocolRequest {
    protocol_id: ProtocolId,
    definition_name: ProtocolName,
    definition_version: ProtocolVersion,
    context: ProtocolContext,
    actor_id: String,
    actor_kind: AuditActorKind,
    limits: Option<made_core::value_objects::BudgetLimits>,
}

impl EmbeddedStartPublishedProtocolRequest {
    pub(super) async fn execute(self, made: &EmbeddedMade) -> Result<ProtocolId, ToolError> {
        let input = StartProtocolInput::new(
            self.protocol_id,
            self.definition_name,
            self.definition_version,
            self.context,
            self.actor_id,
            self.actor_kind,
        );
        let instance = if let Some(limits) = self.limits {
            made.start_budgeted_published(StartBudgetedProtocolInput::new(input, limits))
                .await?
        } else {
            made.start_published(input).await?
        };
        Ok(instance.id().clone())
    }
}

impl TryFrom<&Value> for EmbeddedStartPublishedProtocolRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let object = value
            .as_object()
            .ok_or_else(|| "tools/call.arguments must be an object".to_owned())?;
        let protocol_id =
            optional_string(object, "protocol_id")?.unwrap_or_else(|| Uuid::new_v4().to_string());
        let context = object
            .get("context")
            .map_or_else(|| Ok(ProtocolContext::empty()), context_from_json)?;

        Ok(Self {
            protocol_id: ProtocolId::new(protocol_id).map_err(|error| error.to_string())?,
            definition_name: ProtocolName::new(required_string(object, "protocol")?)
                .map_err(|error| error.to_string())?,
            definition_version: ProtocolVersion::new(required_string(object, "version")?)
                .map_err(|error| error.to_string())?,
            context,
            actor_id: required_string(object, "actor_id")?,
            actor_kind: required_actor_kind(object, "actor_kind")?,
            limits: super::embedded_budget_fields::limits(object)?,
        })
    }
}
