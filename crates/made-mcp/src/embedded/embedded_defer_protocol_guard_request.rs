use made_app::usecases::DeferProtocolGuardInput;
use made_core::value_objects::{
    AuditActorKind, GuardName, ProtocolGuardDeferralContent, ProtocolId, RoleId,
};
use made_embedded::EmbeddedMade;
use serde_json::Value;

use super::embedded_request_fields::{required_actor_kind, required_string, required_strings};

use crate::mcp_protocol::ToolError;

/// Validated MCP request for one explicit human guard deferral.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EmbeddedDeferProtocolGuardRequest {
    protocol_id: ProtocolId,
    guard_name: GuardName,
    content: ProtocolGuardDeferralContent,
    role_id: RoleId,
    role_kind: AuditActorKind,
}

impl EmbeddedDeferProtocolGuardRequest {
    pub(super) async fn execute(self, made: &EmbeddedMade) -> Result<ProtocolId, ToolError> {
        made.defer_guard(DeferProtocolGuardInput::new(
            self.protocol_id.clone(),
            self.guard_name,
            self.content,
            self.role_id,
            self.role_kind,
        ))
        .await?;
        Ok(self.protocol_id)
    }
}

impl TryFrom<&Value> for EmbeddedDeferProtocolGuardRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let object = value
            .as_object()
            .ok_or_else(|| "tools/call.arguments must be an object".to_owned())?;
        Ok(Self {
            protocol_id: ProtocolId::new(required_string(object, "protocol_id")?)
                .map_err(|error| error.to_string())?,
            guard_name: GuardName::new(required_string(object, "guard_name")?)
                .map_err(|error| error.to_string())?,
            content: ProtocolGuardDeferralContent::new(
                required_string(object, "statement")?,
                required_string(object, "reason")?,
                required_strings(object, "reconsider_when")?,
            )
            .map_err(|error| error.to_string())?,
            role_id: RoleId::new(required_string(object, "role_id")?)
                .map_err(|error| error.to_string())?,
            role_kind: required_actor_kind(object, "role_kind")?,
        })
    }
}
