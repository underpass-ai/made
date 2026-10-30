use made_app::usecases::ApplyProtocolTransitionInput;
use made_core::value_objects::{AuditActorKind, ProtocolId, TransitionTrigger};
use made_embedded::EmbeddedMade;
use serde_json::Value;

use super::embedded_request_fields::{
    load_instance_definition, required_actor_kind, required_string,
};

use crate::mcp_protocol::ToolError;

/// Validated MCP request that applies one enabled transition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EmbeddedApplyProtocolTransitionRequest {
    protocol_id: ProtocolId,
    trigger: TransitionTrigger,
    actor_kind: AuditActorKind,
}

impl EmbeddedApplyProtocolTransitionRequest {
    pub(super) async fn execute(self, made: &EmbeddedMade) -> Result<ProtocolId, ToolError> {
        let (definition, _instance) = load_instance_definition(made, &self.protocol_id).await?;
        // The seat is the definition's to say; what filled it is the
        // caller's, and taking both from the definition would record a
        // kind nobody declared.
        let role_id = definition.role_id_for_transition(&self.trigger)?;
        made.apply_transition(ApplyProtocolTransitionInput::new(
            self.protocol_id.clone(),
            role_id,
            self.actor_kind,
            self.trigger,
        ))
        .await?;
        Ok(self.protocol_id)
    }
}

impl TryFrom<&Value> for EmbeddedApplyProtocolTransitionRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let object = value
            .as_object()
            .ok_or_else(|| "tools/call.arguments must be an object".to_owned())?;
        Ok(Self {
            protocol_id: ProtocolId::new(required_string(object, "protocol_id")?)
                .map_err(|error| error.to_string())?,
            trigger: TransitionTrigger::new(required_string(object, "trigger")?)
                .map_err(|error| error.to_string())?,
            actor_kind: required_actor_kind(object, "actor_kind")?,
        })
    }
}
