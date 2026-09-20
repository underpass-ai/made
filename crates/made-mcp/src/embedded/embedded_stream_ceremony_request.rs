use made_app::usecases::StreamCeremonyInput;
use made_core::value_objects::{
    CeremonyAgentExecutionId, CeremonyEventPageLimit, CeremonyId, CeremonyProgressWait, RoleId,
    StepId, StreamVersion,
};
use serde_json::Value;

use super::embedded_request_fields::{optional_string, optional_u64, required_string};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EmbeddedStreamCeremonyRequest {
    ceremony_id: CeremonyId,
    after_sequence: StreamVersion,
    max_events: Option<usize>,
    wait_timeout_ms: Option<u32>,
    include_agent_activity: bool,
    after_activity_sequence: u64,
    role_id: Option<RoleId>,
    step_id: Option<StepId>,
    agent_execution_id: Option<CeremonyAgentExecutionId>,
}

impl EmbeddedStreamCeremonyRequest {
    pub(super) fn into_input(self) -> Result<StreamCeremonyInput, String> {
        let max_events = self
            .max_events
            .map(CeremonyEventPageLimit::new)
            .transpose()
            .map_err(|error| error.to_string())?
            .unwrap_or_default();
        let wait_timeout = CeremonyProgressWait::from_millis(
            self.wait_timeout_ms
                .unwrap_or(CeremonyProgressWait::DEFAULT.millis()),
        )
        .map_err(|error| error.to_string())?;
        let input = StreamCeremonyInput::new(
            self.ceremony_id,
            self.after_sequence,
            max_events,
            wait_timeout,
        );
        Ok(if self.include_agent_activity {
            input.with_agent_activity(
                self.after_activity_sequence,
                self.role_id,
                self.step_id,
                self.agent_execution_id,
            )
        } else {
            input
        })
    }
}

impl TryFrom<&Value> for EmbeddedStreamCeremonyRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let object = value
            .as_object()
            .ok_or_else(|| "tools/call.arguments must be an object".to_owned())?;
        let max_events = optional_u64(object, "max_events")?
            .filter(|value| *value > 0)
            .map(|value| usize::try_from(value).unwrap_or(usize::MAX));
        let wait_timeout_ms = optional_u64(object, "wait_timeout_ms")?
            .map(|value| u32::try_from(value).unwrap_or(u32::MAX));
        Ok(Self {
            ceremony_id: CeremonyId::new(required_string(object, "ceremony_id")?)
                .map_err(|error| error.to_string())?,
            after_sequence: StreamVersion::new(
                optional_u64(object, "after_sequence")?.unwrap_or(0),
            ),
            max_events,
            wait_timeout_ms,
            include_agent_activity: object
                .get("include_agent_activity")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            after_activity_sequence: optional_u64(object, "after_activity_sequence")?.unwrap_or(0),
            role_id: optional_string(object, "role_id")?
                .map(RoleId::new)
                .transpose()
                .map_err(|error| error.to_string())?,
            step_id: optional_string(object, "step_id")?
                .map(StepId::new)
                .transpose()
                .map_err(|error| error.to_string())?,
            agent_execution_id: optional_string(object, "agent_execution_id")?
                .map(CeremonyAgentExecutionId::new)
                .transpose()
                .map_err(|error| error.to_string())?,
        })
    }
}
