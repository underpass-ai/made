use made_app::usecases::CollectProtocolEvidenceInput;
use made_core::value_objects::{
    AuditActorKind, ProtocolEvidenceSourceId, ProtocolId, ProtocolInterventionContent,
    ProtocolInterventionId, RoleId,
};
use made_embedded::EmbeddedMade;
use serde_json::Value;

use super::embedded_request_fields::{optional_attributes, required_actor_kind, required_string};

use crate::mcp_protocol::ToolError;

/// Validated MCP request that collects evidence for one open intervention.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EmbeddedCollectProtocolEvidenceRequest {
    protocol_id: ProtocolId,
    intervention_id: ProtocolInterventionId,
    role_id: RoleId,
    role_kind: AuditActorKind,
    source_id: ProtocolEvidenceSourceId,
    query: ProtocolInterventionContent,
}

impl EmbeddedCollectProtocolEvidenceRequest {
    pub(super) async fn execute(self, made: &EmbeddedMade) -> Result<ProtocolId, ToolError> {
        made.collect_evidence(CollectProtocolEvidenceInput::new(
            self.protocol_id.clone(),
            self.intervention_id,
            self.role_id,
            self.role_kind,
            self.source_id,
            self.query,
        ))
        .await?;
        Ok(self.protocol_id)
    }
}

impl TryFrom<&Value> for EmbeddedCollectProtocolEvidenceRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let object = value
            .as_object()
            .ok_or_else(|| "tools/call.arguments must be an object".to_owned())?;
        Ok(Self {
            protocol_id: ProtocolId::new(required_string(object, "protocol_id")?)
                .map_err(|error| error.to_string())?,
            intervention_id: ProtocolInterventionId::new(required_string(
                object,
                "intervention_id",
            )?)
            .map_err(|error| error.to_string())?,
            role_kind: required_actor_kind(object, "role_kind")?,
            role_id: RoleId::new(required_string(object, "role_id")?)
                .map_err(|error| error.to_string())?,
            source_id: ProtocolEvidenceSourceId::new(required_string(object, "source_id")?)
                .map_err(|error| error.to_string())?,
            query: ProtocolInterventionContent::new(
                required_string(object, "query")?,
                optional_attributes(object, "details")?,
            )
            .map_err(|error| error.to_string())?,
        })
    }
}
