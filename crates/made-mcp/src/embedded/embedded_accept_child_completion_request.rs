use made_app::usecases::AcceptChildCompletionInput;
use made_core::value_objects::{CeremonyId, EventId};
use made_embedded::EmbeddedMade;
use serde_json::{json, Value};

use super::embedded_ceremony_instance_presenter::{
    child_completion_value, EmbeddedCeremonyInstancePresenter,
};
use super::embedded_request_fields::required_string;
use crate::protocol::ToolError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EmbeddedAcceptChildCompletionRequest {
    child_id: CeremonyId,
    terminal_event_id: EventId,
}

impl EmbeddedAcceptChildCompletionRequest {
    pub(super) async fn execute(self, made: &EmbeddedMade) -> Result<Value, ToolError> {
        let output = made
            .accept_child_completion(AcceptChildCompletionInput::new(
                self.child_id,
                self.terminal_event_id,
            ))
            .await?;
        let parent = EmbeddedCeremonyInstancePresenter::present(made, output.parent().id()).await?;
        Ok(json!({
            "parent": parent,
            "completion": child_completion_value(output.completion()),
        }))
    }
}

impl TryFrom<&Value> for EmbeddedAcceptChildCompletionRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let object = value
            .as_object()
            .ok_or_else(|| "tools/call.arguments must be an object".to_owned())?;
        Ok(Self {
            child_id: CeremonyId::new(required_string(object, "child_id")?)
                .map_err(|error| error.to_string())?,
            terminal_event_id: EventId::new(required_string(object, "terminal_event_id")?)
                .map_err(|error| error.to_string())?,
        })
    }
}
