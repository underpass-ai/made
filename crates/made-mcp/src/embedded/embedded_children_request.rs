use made_app::usecases::AcceptChildCompletionInput;
use made_core::value_objects::{
    CeremonyEventPageLimit, CeremonyId, ChildGroupId, ChildSpawnCoordinates, EventId,
};
use made_embedded::EmbeddedMade;
use serde_json::{json, Value};

use super::embedded_ceremony_instance_presenter::{
    child_completion_value, EmbeddedCeremonyInstancePresenter,
};
use super::embedded_request_fields::{optional_u64, required_string};
use super::embedded_run_ceremony_step_request::EmbeddedRunCeremonyStepRequest;
use crate::protocol::ToolError;

#[derive(Clone, Debug)]
pub(super) struct EmbeddedPrepareCeremonyChildrenRequest {
    run: EmbeddedRunCeremonyStepRequest,
}

impl EmbeddedPrepareCeremonyChildrenRequest {
    pub(super) async fn execute(self, made: &EmbeddedMade) -> Result<Value, ToolError> {
        let step_id = self.run.step_id().clone();
        let output = self.run.execute_output(made).await?;
        let record = output
            .instance()
            .step_record(&step_id)
            .ok_or_else(|| ToolError::refused("made returned no spawning step record"))?;
        let coordinates = ChildSpawnCoordinates::new(
            step_id,
            output.instance().current_state_visit(),
            output.instance().current_state_iteration(),
            record.iteration(),
        );
        let group_id = ChildGroupId::derive(output.instance().id(), &coordinates);
        let group = output
            .instance()
            .child_group(&group_id)
            .ok_or_else(|| ToolError::refused("made returned no child spawn group"))?;
        let instance =
            EmbeddedCeremonyInstancePresenter::present(made, output.instance().id()).await?;
        Ok(json!({
            "instance": instance,
            "child_group_id": group_id.as_str(),
            "child_ids": group.plan().children().iter().map(|child| child.child_id().as_str()).collect::<Vec<_>>(),
        }))
    }
}

impl TryFrom<&Value> for EmbeddedPrepareCeremonyChildrenRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        Ok(Self {
            run: EmbeddedRunCeremonyStepRequest::try_from(value)?,
        })
    }
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct EmbeddedRecoverCeremonyChildrenRequest {
    limit: CeremonyEventPageLimit,
}

impl EmbeddedRecoverCeremonyChildrenRequest {
    pub(super) async fn execute(self, made: &EmbeddedMade) -> Result<Value, ToolError> {
        let round = made.recover_children(self.limit).await?;
        Ok(json!({
            "recovered_plans": round.recovered_plans,
            "accepted_completions": round.accepted_completions,
            "skipped": round.skipped,
            "failed": round.failed,
            "busy": round.busy,
        }))
    }
}

impl TryFrom<&Value> for EmbeddedRecoverCeremonyChildrenRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let object = value
            .as_object()
            .ok_or_else(|| "tools/call.arguments must be an object".to_owned())?;
        let requested = optional_u64(object, "limit")?.unwrap_or_default();
        let limit = if requested == 0 {
            CeremonyEventPageLimit::DEFAULT
        } else {
            CeremonyEventPageLimit::new(
                usize::try_from(requested).map_err(|_| "field `limit` is too large".to_owned())?,
            )
            .map_err(|error| error.to_string())?
        };
        Ok(Self { limit })
    }
}
