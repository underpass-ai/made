//! Reading the two catalogue requests off the wire.

use made_mcp_proto::v1 as pb;
use serde_json::Value;

use super::j2p;

pub(super) fn list(arguments: &Value) -> Result<pb::ListCeremonyDefinitionsRequest, String> {
    let object = j2p::require_object(arguments, "tools/call.arguments")?;
    let limit = object.get("limit").and_then(Value::as_u64).unwrap_or(0);
    Ok(pb::ListCeremonyDefinitionsRequest {
        ceremony: j2p::optional_str(object, "ceremony")
            .unwrap_or_default()
            .to_owned(),
        limit: u32::try_from(limit).map_err(|_| "field `limit` is out of range".to_owned())?,
        cursor: j2p::optional_str(object, "cursor")
            .unwrap_or_default()
            .to_owned(),
    })
}

pub(super) fn get(arguments: &Value) -> Result<pb::GetCeremonyDefinitionRequest, String> {
    let object = j2p::require_object(arguments, "tools/call.arguments")?;
    Ok(pb::GetCeremonyDefinitionRequest {
        ceremony: j2p::require_str(object, "ceremony")?.to_owned(),
        version: j2p::require_str(object, "version")?.to_owned(),
    })
}
