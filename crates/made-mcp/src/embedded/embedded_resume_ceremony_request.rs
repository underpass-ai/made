use made_app::usecases::ResumeCeremonyInput;
use made_core::value_objects::CeremonyId;
use made_embedded::EmbeddedMade;
use serde_json::Value;

use super::embedded_request_fields::{required_actor_kind, required_string};
use crate::protocol::ToolError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EmbeddedResumeCeremonyRequest {
    input: ResumeCeremonyInput,
}

impl EmbeddedResumeCeremonyRequest {
    pub(super) async fn execute(self, made: &EmbeddedMade) -> Result<CeremonyId, ToolError> {
        let id = self.input.instance_id().clone();
        made.resume_ceremony(self.input).await?;
        Ok(id)
    }
}

impl TryFrom<&Value> for EmbeddedResumeCeremonyRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let object = value
            .as_object()
            .ok_or_else(|| "tools/call.arguments must be an object".to_owned())?;
        Ok(Self {
            input: ResumeCeremonyInput::new(
                CeremonyId::new(required_string(object, "ceremony_id")?)
                    .map_err(|error| error.to_string())?,
                required_string(object, "actor_id")?,
                required_actor_kind(object, "actor_kind")?,
            ),
        })
    }
}
