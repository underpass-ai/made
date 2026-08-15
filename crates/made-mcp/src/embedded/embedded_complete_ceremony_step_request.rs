use made_app::usecases::CompleteCeremonyStepInput;
use made_core::value_objects::{
    AuditActorKind, CeremonyId, StepErrorMessage, StepId, StepOutput, StepResult, StepStatus,
};
use made_embedded::EmbeddedMade;
use serde_json::Value;

use super::embedded_request_fields::{
    optional_attributes, optional_string, required_actor_kind, required_string,
};

/// Validated MCP request that records one host-executed step result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EmbeddedCompleteCeremonyStepRequest {
    ceremony_id: CeremonyId,
    step_id: StepId,
    actor_kind: AuditActorKind,
    result: StepResult,
}

impl EmbeddedCompleteCeremonyStepRequest {
    pub(super) async fn execute(self, made: &EmbeddedMade) -> Result<CeremonyId, String> {
        made.complete_step(CompleteCeremonyStepInput::new(
            self.ceremony_id.clone(),
            self.step_id,
            self.result,
            self.actor_kind,
        ))
        .await
        .map_err(|error| format!("failed to complete ceremony step: {error}"))?;
        Ok(self.ceremony_id)
    }
}

impl TryFrom<&Value> for EmbeddedCompleteCeremonyStepRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let object = value
            .as_object()
            .ok_or_else(|| "tools/call.arguments must be an object".to_owned())?;
        let status = parse_status(&required_string(object, "status")?)?;
        let output = StepOutput::new(optional_attributes(object, "output")?);
        let error = optional_string(object, "error")?
            .map(StepErrorMessage::new)
            .transpose()
            .map_err(|error| error.to_string())?;
        let result = StepResult::new(status, output, error).map_err(|error| error.to_string())?;

        Ok(Self {
            ceremony_id: CeremonyId::new(required_string(object, "ceremony_id")?)
                .map_err(|error| error.to_string())?,
            step_id: StepId::new(required_string(object, "step_id")?)
                .map_err(|error| error.to_string())?,
            actor_kind: required_actor_kind(object, "actor_kind")?,
            result,
        })
    }
}

fn parse_status(value: &str) -> Result<StepStatus, String> {
    match value {
        "completed" => Ok(StepStatus::Completed),
        "failed" => Ok(StepStatus::Failed),
        "waiting_for_human" => Ok(StepStatus::WaitingForHuman),
        "cancelled" => Ok(StepStatus::Cancelled),
        other => Err(format!(
            "`status` must be completed, failed, waiting_for_human or cancelled, not {other}"
        )),
    }
}
