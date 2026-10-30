use made_app::usecases::StartProtocolInput;
use made_core::value_objects::{AuditActorKind, ProtocolContext, ProtocolId};
use made_embedded::EmbeddedMade;
use serde_json::Value;
use uuid::Uuid;

use super::embedded_request_fields::{
    context_from_json, optional_string, required_actor_kind, required_string,
};

use crate::mcp_protocol::ToolError;

/// Validated MCP request that starts, but does not advance, a protocol.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EmbeddedStartProtocolRequest {
    definition_yaml: String,
    protocol_id: ProtocolId,
    context: ProtocolContext,
    actor_id: String,
    actor_kind: AuditActorKind,
}

impl EmbeddedStartProtocolRequest {
    pub(super) async fn execute(self, made: &EmbeddedMade) -> Result<ProtocolId, ToolError> {
        let mounted = made.mount_yaml(&self.definition_yaml).await?;
        let definition = mounted.definitions().first().ok_or_else(|| {
            ToolError::refused("protocol definition source returned no definitions")
        })?;
        let instance = made
            .start(StartProtocolInput::new(
                self.protocol_id,
                definition.name().clone(),
                definition.version().clone(),
                self.context,
                self.actor_id,
                self.actor_kind,
            ))
            .await?;
        Ok(instance.id().clone())
    }
}

impl TryFrom<&Value> for EmbeddedStartProtocolRequest {
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
            definition_yaml: required_string(object, "definition_yaml")?,
            protocol_id: ProtocolId::new(protocol_id).map_err(|error| error.to_string())?,
            context,
            actor_id: required_string(object, "actor_id")?,
            actor_kind: required_actor_kind(object, "actor_kind")?,
        })
    }
}
