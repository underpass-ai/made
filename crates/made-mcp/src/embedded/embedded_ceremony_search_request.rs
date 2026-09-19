use made_app::usecases::{CeremonySearchCursor, SearchCeremonyInstancesInput};
use made_core::value_objects::{
    AuthorizationRequestId, CeremonyIdPrefix, CeremonyInstancePageLimit, CeremonyLifecyclePhase,
};
use made_embedded::EmbeddedMade;
use serde_json::Value;

use crate::protocol::ToolError;

use super::embedded_ceremony_listing;

#[derive(Debug)]
pub(super) struct EmbeddedCeremonySearchRequest {
    input: SearchCeremonyInstancesInput,
}

impl EmbeddedCeremonySearchRequest {
    pub(super) async fn execute_and_present(
        &self,
        made: &EmbeddedMade,
        request_id: AuthorizationRequestId,
    ) -> Result<Value, ToolError> {
        let page = made.search_instances(request_id, &self.input).await?;
        embedded_ceremony_listing::present_page(made, &page).await
    }
}

impl TryFrom<&Value> for EmbeddedCeremonySearchRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let object = value
            .as_object()
            .ok_or_else(|| "tools/call.arguments: expected an object".to_owned())?;
        let cursor = object
            .get("cursor")
            .map(|value| {
                value
                    .as_str()
                    .ok_or_else(|| "`cursor` must be a string".to_owned())
                    .and_then(|value| {
                        CeremonySearchCursor::parse(value.to_owned())
                            .map_err(|error| error.to_string())
                    })
            })
            .transpose()?;
        let limit = match object.get("limit") {
            None => CeremonyInstancePageLimit::default(),
            Some(value) => {
                let value = value
                    .as_u64()
                    .and_then(|value| u16::try_from(value).ok())
                    .ok_or_else(|| "`limit` must be a positive integer".to_owned())?;
                CeremonyInstancePageLimit::new(value).map_err(|error| error.to_string())?
            }
        };
        let id_prefix = object
            .get("id_prefix")
            .map(|value| {
                value
                    .as_str()
                    .ok_or_else(|| "`id_prefix` must be a string".to_owned())
                    .and_then(|value| {
                        CeremonyIdPrefix::new(value.to_owned()).map_err(|error| error.to_string())
                    })
            })
            .transpose()?;
        let lifecycle = match object.get("lifecycle") {
            None => None,
            Some(Value::String(value)) if value == "running" => {
                Some(CeremonyLifecyclePhase::Running)
            }
            Some(Value::String(value)) if value == "paused" => Some(CeremonyLifecyclePhase::Paused),
            Some(Value::String(value)) if value == "ended" => Some(CeremonyLifecyclePhase::Ended),
            Some(_) => {
                return Err("`lifecycle` must be running, paused or ended".to_owned());
            }
        };
        Ok(Self {
            input: SearchCeremonyInstancesInput::new(cursor, limit, id_prefix, lifecycle),
        })
    }
}
