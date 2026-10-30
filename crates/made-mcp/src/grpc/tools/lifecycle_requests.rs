use made_mcp_proto::v1 as pb;
use serde_json::Value;

use super::super::json_to_proto as j2p;

pub(super) fn build_pause_protocol_request(
    args: &Value,
) -> Result<pb::PauseProtocolRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::PauseProtocolRequest {
        protocol_id: j2p::require_str(obj, "protocol_id")?.to_owned(),
        actor_id: j2p::require_str(obj, "actor_id")?.to_owned(),
        actor_kind: j2p::require_str(obj, "actor_kind")?.to_owned(),
        reason: j2p::require_str(obj, "reason")?.to_owned(),
    })
}

pub(super) fn build_resume_protocol_request(
    args: &Value,
) -> Result<pb::ResumeProtocolRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::ResumeProtocolRequest {
        protocol_id: j2p::require_str(obj, "protocol_id")?.to_owned(),
        actor_id: j2p::require_str(obj, "actor_id")?.to_owned(),
        actor_kind: j2p::require_str(obj, "actor_kind")?.to_owned(),
    })
}

pub(super) fn build_cancel_protocol_request(
    args: &Value,
) -> Result<pb::CancelProtocolRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::CancelProtocolRequest {
        protocol_id: j2p::require_str(obj, "protocol_id")?.to_owned(),
        actor_id: j2p::require_str(obj, "actor_id")?.to_owned(),
        actor_kind: j2p::require_str(obj, "actor_kind")?.to_owned(),
        reason: j2p::require_str(obj, "reason")?.to_owned(),
    })
}

pub(super) fn build_enforce_protocol_deadlines_request(
    args: &Value,
) -> Result<pb::EnforceProtocolDeadlinesRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::EnforceProtocolDeadlinesRequest {
        protocol_id: j2p::require_str(obj, "protocol_id")?.to_owned(),
    })
}
