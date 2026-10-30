use made_app::usecases::{RunProtocolInput, RunProtocolOutput};
use made_core::value_objects::{
    AuditActorKind, DurationMs, LeaseOwnerId, ProtocolContext, ProtocolId,
};
use made_embedded::EmbeddedMade;
use serde_json::Value;
use uuid::Uuid;

use super::embedded_request_fields::{
    context_from_json, optional_string, optional_u64, required_actor_kind, required_string,
};

use crate::embedded::EMBEDDED_BACKEND_NAME;
use crate::mcp_protocol::{default_lease_owner_id, ToolError, RUN_PROTOCOL_LEASE_TTL_MS};

/// Validated MCP request for an embedded protocol execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EmbeddedRunProtocolRequest {
    definition_yaml: String,
    protocol_id: ProtocolId,
    context: ProtocolContext,
    lease_owner_id: LeaseOwnerId,
    lease_ttl: DurationMs,
    actor_id: String,
    actor_kind: AuditActorKind,
}

impl EmbeddedRunProtocolRequest {
    pub(crate) async fn execute(self, made: &EmbeddedMade) -> Result<RunProtocolOutput, ToolError> {
        let mounted = made.mount_yaml(&self.definition_yaml).await?;
        let definition = mounted.definitions().first().cloned().ok_or_else(|| {
            ToolError::refused("protocol definition source returned no definitions")
        })?;

        Box::pin(made.run(RunProtocolInput::new(
            self.protocol_id,
            definition,
            self.context,
            self.lease_owner_id,
            self.lease_ttl,
            self.actor_id,
            self.actor_kind,
        )))
        .await
        .map_err(ToolError::from)
    }
}

impl TryFrom<&Value> for EmbeddedRunProtocolRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let object = value
            .as_object()
            .ok_or_else(|| "tools/call.arguments must be an object".to_owned())?;
        let definition_yaml = required_string(object, "definition_yaml")?;
        let protocol_id =
            optional_string(object, "protocol_id")?.unwrap_or_else(|| Uuid::new_v4().to_string());
        let lease_owner_id = optional_string(object, "lease_owner_id")?
            .unwrap_or_else(|| default_lease_owner_id(EMBEDDED_BACKEND_NAME));
        let lease_ttl_ms = optional_u64(object, "lease_ttl_ms")?.unwrap_or_default();
        let context = object
            .get("context")
            .map_or_else(|| Ok(ProtocolContext::empty()), context_from_json)?;

        Ok(Self {
            definition_yaml,
            protocol_id: ProtocolId::new(protocol_id).map_err(|error| error.to_string())?,
            context,
            lease_owner_id: LeaseOwnerId::new(lease_owner_id).map_err(|error| error.to_string())?,
            lease_ttl: DurationMs::from_millis(if lease_ttl_ms == 0 {
                RUN_PROTOCOL_LEASE_TTL_MS
            } else {
                lease_ttl_ms
            }),
            actor_id: required_string(object, "actor_id")?,
            actor_kind: required_actor_kind(object, "actor_kind")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The lease a one-shot run takes is the same length on both arms.
    ///
    /// It was thirty seconds here and sixty over the wire, because this
    /// arm chose a number and the other sent a zero for the server to
    /// choose with. One omission now means one lease.
    #[test]
    fn an_omitted_lease_length_is_the_one_both_arms_apply() {
        let request = EmbeddedRunProtocolRequest::try_from(&json!({
            "definition_yaml": "version: \"1.0\"",
            "actor_id": "operator",
            "actor_kind": "service",
        }))
        .expect("the request should be accepted");

        assert_eq!(request.lease_ttl.get(), RUN_PROTOCOL_LEASE_TTL_MS);
        assert_eq!(request.lease_owner_id.as_str(), "made-mcp:embedded");
    }

    #[test]
    fn a_lease_length_the_caller_asked_for_is_left_alone() {
        let request = EmbeddedRunProtocolRequest::try_from(&json!({
            "definition_yaml": "version: \"1.0\"",
            "actor_id": "operator",
            "actor_kind": "service",
            "lease_ttl_ms": 1_500,
        }))
        .expect("the request should be accepted");

        assert_eq!(request.lease_ttl.get(), 1_500);
    }
}
