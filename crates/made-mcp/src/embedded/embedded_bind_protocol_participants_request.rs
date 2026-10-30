use std::collections::BTreeMap;

use made_app::usecases::BindProtocolParticipantsInput;
use made_core::entities::ProtocolInstance;
use made_core::value_objects::{ProtocolId, RoleId, Specialty};
use made_embedded::EmbeddedMade;
use serde_json::Value;

use super::embedded_request_fields::{required_actor_kind, required_string};

use crate::mcp_protocol::ToolError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EmbeddedBindProtocolParticipantsRequest {
    input: BindProtocolParticipantsInput,
}

impl EmbeddedBindProtocolParticipantsRequest {
    pub(super) async fn execute(self, made: &EmbeddedMade) -> Result<ProtocolInstance, ToolError> {
        made.bind_participants(self.input)
            .await
            .map_err(ToolError::from)
    }
}

impl TryFrom<&Value> for EmbeddedBindProtocolParticipantsRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let object = value
            .as_object()
            .ok_or_else(|| "tools/call.arguments must be an object".to_owned())?;
        let seating = object
            .get("seating")
            .and_then(Value::as_object)
            .ok_or_else(|| "field `seating` must be an object".to_owned())?;

        let mut seats = BTreeMap::new();
        for (role, specialty) in seating {
            let specialty = specialty
                .as_str()
                .ok_or_else(|| format!("`seating.{role}` must be a string"))?;
            seats.insert(
                RoleId::new(role.clone()).map_err(|error| error.to_string())?,
                Specialty::new(specialty).map_err(|error| error.to_string())?,
            );
        }

        Ok(Self {
            input: BindProtocolParticipantsInput::new(
                ProtocolId::new(required_string(object, "protocol_id")?)
                    .map_err(|error| error.to_string())?,
                seats,
                required_string(object, "actor_id")?,
                required_actor_kind(object, "actor_kind")?,
            )
            .map_err(|error| error.to_string())?,
        })
    }
}
