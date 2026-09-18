use made_mcp_proto::v1 as pb;
use serde_json::Value;

use super::super::json_to_proto as j2p;

pub(super) fn build_pause_ceremony_request(
    args: &Value,
) -> Result<pb::PauseCeremonyRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::PauseCeremonyRequest {
        ceremony_id: j2p::require_str(obj, "ceremony_id")?.to_owned(),
        actor_id: j2p::require_str(obj, "actor_id")?.to_owned(),
        actor_kind: j2p::require_str(obj, "actor_kind")?.to_owned(),
        reason: j2p::require_str(obj, "reason")?.to_owned(),
    })
}

pub(super) fn build_resume_ceremony_request(
    args: &Value,
) -> Result<pb::ResumeCeremonyRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::ResumeCeremonyRequest {
        ceremony_id: j2p::require_str(obj, "ceremony_id")?.to_owned(),
        actor_id: j2p::require_str(obj, "actor_id")?.to_owned(),
        actor_kind: j2p::require_str(obj, "actor_kind")?.to_owned(),
    })
}

pub(super) fn build_cancel_ceremony_request(
    args: &Value,
) -> Result<pb::CancelCeremonyRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::CancelCeremonyRequest {
        ceremony_id: j2p::require_str(obj, "ceremony_id")?.to_owned(),
        actor_id: j2p::require_str(obj, "actor_id")?.to_owned(),
        actor_kind: j2p::require_str(obj, "actor_kind")?.to_owned(),
        reason: j2p::require_str(obj, "reason")?.to_owned(),
    })
}

pub(super) fn build_enforce_ceremony_deadlines_request(
    args: &Value,
) -> Result<pb::EnforceCeremonyDeadlinesRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::EnforceCeremonyDeadlinesRequest {
        ceremony_id: j2p::require_str(obj, "ceremony_id")?.to_owned(),
    })
}
