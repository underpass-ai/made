use made_app::usecases::ResumeProtocolInput;
use made_core::value_objects::ProtocolId;
use made_embedded::EmbeddedMade;
use serde_json::Value;

use super::embedded_request_fields::{required_actor_kind, required_string};
use crate::mcp_protocol::ToolError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EmbeddedResumeProtocolRequest {
    input: ResumeProtocolInput,
}

impl EmbeddedResumeProtocolRequest {
    pub(super) async fn execute(self, made: &EmbeddedMade) -> Result<ProtocolId, ToolError> {
        let id = self.input.instance_id().clone();
        made.resume_protocol(self.input).await?;
        Ok(id)
    }
}

impl TryFrom<&Value> for EmbeddedResumeProtocolRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let object = value
            .as_object()
            .ok_or_else(|| "tools/call.arguments must be an object".to_owned())?;
        Ok(Self {
            input: ResumeProtocolInput::new(
                ProtocolId::new(required_string(object, "protocol_id")?)
                    .map_err(|error| error.to_string())?,
                required_string(object, "actor_id")?,
                required_actor_kind(object, "actor_kind")?,
            ),
        })
    }
}
