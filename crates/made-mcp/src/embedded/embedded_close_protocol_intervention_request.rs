use made_app::usecases::CloseProtocolInterventionInput;
use made_core::value_objects::{AuditActorKind, ProtocolId, ProtocolInterventionId, RoleId};
use made_embedded::EmbeddedMade;
use serde_json::Value;

use super::embedded_request_fields::{required_actor_kind, required_string};

use crate::mcp_protocol::ToolError;

/// Validated MCP request that closes a dynamic protocol intervention.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EmbeddedCloseProtocolInterventionRequest {
    protocol: ProtocolId,
    intervention: ProtocolInterventionId,
    role: RoleId,
    role_kind: AuditActorKind,
}

impl EmbeddedCloseProtocolInterventionRequest {
    pub(super) async fn execute(self, made: &EmbeddedMade) -> Result<ProtocolId, ToolError> {
        made.close_intervention(CloseProtocolInterventionInput::new(
            self.protocol.clone(),
            self.intervention,
            self.role,
            self.role_kind,
        ))
        .await?;
        Ok(self.protocol)
    }
}

impl TryFrom<&Value> for EmbeddedCloseProtocolInterventionRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let object = value
            .as_object()
            .ok_or_else(|| "tools/call.arguments must be an object".to_owned())?;
        Ok(Self {
            protocol: ProtocolId::new(required_string(object, "protocol_id")?)
                .map_err(|error| error.to_string())?,
            intervention: ProtocolInterventionId::new(required_string(object, "intervention_id")?)
                .map_err(|error| error.to_string())?,
            role_kind: required_actor_kind(object, "role_kind")?,
            role: RoleId::new(required_string(object, "role_id")?)
                .map_err(|error| error.to_string())?,
        })
    }
}
