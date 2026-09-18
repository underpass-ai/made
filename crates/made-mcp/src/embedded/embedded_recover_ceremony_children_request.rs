use made_core::value_objects::CeremonyEventPageLimit;
use made_embedded::EmbeddedMade;
use serde_json::{json, Value};

use super::embedded_request_fields::optional_u64;
use crate::protocol::ToolError;

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
