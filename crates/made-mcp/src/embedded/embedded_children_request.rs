use made_core::value_objects::{ChildGroupId, ChildSpawnCoordinates};
use made_embedded::EmbeddedMade;
use serde_json::{json, Value};

use super::embedded_ceremony_instance_presenter::EmbeddedCeremonyInstancePresenter;
use super::embedded_run_ceremony_step_request::EmbeddedRunCeremonyStepRequest;
use crate::protocol::ToolError;

#[derive(Clone, Debug)]
pub(super) struct EmbeddedPrepareCeremonyChildrenRequest {
    run: EmbeddedRunCeremonyStepRequest,
}

impl EmbeddedPrepareCeremonyChildrenRequest {
    pub(super) async fn execute(self, made: &EmbeddedMade) -> Result<Value, ToolError> {
        let step_id = self.run.step_id().clone();
        let output = Box::pin(self.run.execute_output(made)).await?;
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
