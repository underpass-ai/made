use made_core::value_objects::{CeremonyEventConsumer, CeremonyEventPageLimit, GlobalPosition};
use serde_json::Value;

use super::embedded_request_fields::{optional_u64, required_string};

/// Validated request for a named global ceremony-event feed.
pub(super) struct EmbeddedPullCeremonyEventsRequest {
    pub consumer: CeremonyEventConsumer,
    pub limit: CeremonyEventPageLimit,
    pub acknowledge_through: Option<GlobalPosition>,
}

impl TryFrom<&Value> for EmbeddedPullCeremonyEventsRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let object = value
            .as_object()
            .ok_or_else(|| "tools/call.arguments must be an object".to_owned())?;
        let consumer = CeremonyEventConsumer::new(required_string(object, "consumer")?)
            .map_err(|error| error.to_string())?;
        let limit = optional_u64(object, "limit")?.unwrap_or(0);
        let limit = if limit == 0 {
            CeremonyEventPageLimit::DEFAULT
        } else {
            CeremonyEventPageLimit::new(usize::try_from(limit).unwrap_or(usize::MAX))
                .map_err(|error| error.to_string())?
        };
        let acknowledge_through = optional_u64(object, "acknowledge_through")?
            .map(GlobalPosition::new)
            .transpose()
            .map_err(|error| error.to_string())?;
        Ok(Self {
            consumer,
            limit,
            acknowledge_through,
        })
    }
}
