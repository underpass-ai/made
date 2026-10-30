use made_core::value_objects::ProtocolId;
use serde_json::Value;

use super::embedded_request_fields::required_string;

/// Validated arguments of a tool whose whole request is one protocol
/// id: the session projection and the transcript read.
///
/// One type rather than one per tool, because there is one rule —
/// `protocol_id` is required and has to be a well-formed id — and two
/// copies of it would be two places for it to drift.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EmbeddedProtocolIdRequest {
    protocol_id: ProtocolId,
}

impl EmbeddedProtocolIdRequest {
    pub(super) fn into_protocol_id(self) -> ProtocolId {
        self.protocol_id
    }
}

impl TryFrom<&Value> for EmbeddedProtocolIdRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let object = value
            .as_object()
            .ok_or_else(|| "tools/call.arguments must be an object".to_owned())?;
        Ok(Self {
            protocol_id: ProtocolId::new(required_string(object, "protocol_id")?)
                .map_err(|error| error.to_string())?,
        })
    }
}
