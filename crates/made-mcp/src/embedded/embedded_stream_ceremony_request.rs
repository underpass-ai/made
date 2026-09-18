use made_app::usecases::StreamCeremonyInput;
use made_core::value_objects::{
    CeremonyEventPageLimit, CeremonyId, CeremonyProgressWait, StreamVersion,
};
use serde_json::Value;

use super::embedded_request_fields::{optional_u64, required_string};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EmbeddedStreamCeremonyRequest {
    ceremony_id: CeremonyId,
    after_sequence: StreamVersion,
    max_events: Option<usize>,
    wait_timeout_ms: Option<u32>,
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
        Ok(StreamCeremonyInput::new(
            self.ceremony_id,
            self.after_sequence,
            max_events,
            wait_timeout,
        ))
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
        })
    }
}
