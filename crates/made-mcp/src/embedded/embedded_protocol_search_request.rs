use made_app::usecases::{
    ProtocolInstancePage, ProtocolSearchCursor, SearchProtocolInstancesInput,
};
use made_core::value_objects::{
    AuthorizationRequestId, AuthorizationTargetDigest, ProtocolIdPrefix, ProtocolInstancePageLimit,
    ProtocolLifecyclePhase,
};
use made_embedded::EmbeddedMade;
use serde_json::Value;

use crate::mcp_protocol::ToolError;

#[derive(Debug)]
pub(super) struct EmbeddedProtocolSearchRequest {
    input: SearchProtocolInstancesInput,
}

impl EmbeddedProtocolSearchRequest {
    pub(super) fn authorization_target_digest(&self) -> AuthorizationTargetDigest {
        self.input.authorization_target_digest()
    }

    pub(super) async fn execute(
        &self,
        made: &EmbeddedMade,
        request_id: AuthorizationRequestId,
    ) -> Result<ProtocolInstancePage, ToolError> {
        Ok(made.search_instances(request_id, &self.input).await?)
    }
}

impl TryFrom<&Value> for EmbeddedProtocolSearchRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let object = value
            .as_object()
            .ok_or_else(|| "tools/call.arguments: expected an object".to_owned())?;
        let cursor = object
            .get("cursor")
            .map(|value| {
                value
                    .as_str()
                    .ok_or_else(|| "`cursor` must be a string".to_owned())
                    .and_then(|value| {
                        ProtocolSearchCursor::parse(value.to_owned())
                            .map_err(|error| error.to_string())
                    })
            })
            .transpose()?;
        let limit = match object.get("limit") {
            None => ProtocolInstancePageLimit::default(),
            Some(value) => {
                let value = value
                    .as_u64()
                    .and_then(|value| u16::try_from(value).ok())
                    .ok_or_else(|| "`limit` must be a positive integer".to_owned())?;
                ProtocolInstancePageLimit::new(value).map_err(|error| error.to_string())?
            }
        };
        let id_prefix = object
            .get("id_prefix")
            .map(|value| {
                value
                    .as_str()
                    .ok_or_else(|| "`id_prefix` must be a string".to_owned())
                    .and_then(|value| {
                        ProtocolIdPrefix::new(value.to_owned()).map_err(|error| error.to_string())
                    })
            })
            .transpose()?;
        let lifecycle = match object.get("lifecycle") {
            None => None,
            Some(Value::String(value)) if value == "running" => {
                Some(ProtocolLifecyclePhase::Running)
            }
            Some(Value::String(value)) if value == "paused" => Some(ProtocolLifecyclePhase::Paused),
            Some(Value::String(value)) if value == "ended" => Some(ProtocolLifecyclePhase::Ended),
            Some(_) => {
                return Err("`lifecycle` must be running, paused or ended".to_owned());
            }
        };
        Ok(Self {
            input: SearchProtocolInstancesInput::new(cursor, limit, id_prefix, lifecycle),
        })
    }
}
