use made_core::value_objects::CeremonyId;
use serde_json::Value;

use super::embedded_request_fields::required_string;

/// Validated arguments of a tool whose whole request is one ceremony
/// id: the session projection and the transcript read.
///
/// One type rather than one per tool, because there is one rule —
/// `ceremony_id` is required and has to be a well-formed id — and two
/// copies of it would be two places for it to drift.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EmbeddedCeremonyIdRequest {
    ceremony_id: CeremonyId,
}

impl EmbeddedCeremonyIdRequest {
    pub(super) fn into_ceremony_id(self) -> CeremonyId {
        self.ceremony_id
    }
}

impl TryFrom<&Value> for EmbeddedCeremonyIdRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let object = value
            .as_object()
            .ok_or_else(|| "tools/call.arguments must be an object".to_owned())?;
        Ok(Self {
            ceremony_id: CeremonyId::new(required_string(object, "ceremony_id")?)
                .map_err(|error| error.to_string())?,
        })
    }
}
