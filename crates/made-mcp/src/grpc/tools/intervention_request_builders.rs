//! Building the two intervention verbs that grew optional halves.
//!
//! Split from the rest of the ceremony builders because a request that
//! can address one live agent, carry delivery terms and name a
//! supervisor is no longer the four-field shape its neighbours are.

use made_mcp_proto::v1 as pb;
use serde_json::{Map, Value};

use super::super::json_to_proto as j2p;
use super::ceremony_requests::minted_id;

pub(super) fn build_request_ceremony_intervention_request(
    args: &Value,
) -> Result<pb::RequestCeremonyInterventionRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::RequestCeremonyInterventionRequest {
        ceremony_id: j2p::require_str(obj, "ceremony_id")?.to_owned(),
        intervention_id: minted_id(obj, "intervention_id"),
        role_id: j2p::require_str(obj, "role_id")?.to_owned(),
        role_kind: j2p::require_str(obj, "role_kind")?.to_owned(),
        kind: j2p::require_str(obj, "kind")?.to_owned(),
        target_role_ids: j2p::string_array(obj, "target_role_ids"),
        message: j2p::require_str(obj, "message")?.to_owned(),
        details: j2p::optional_pb_struct(obj, "details")?,
        provenance: provenance_from_json(obj)?,
        target_agent_execution_id: j2p::optional_str(obj, "target_agent_execution_id")
            .unwrap_or_default()
            .to_owned(),
        target_incarnation: j2p::optional_str(obj, "target_incarnation")
            .unwrap_or_default()
            .to_owned(),
        intent: j2p::optional_str(obj, "intent")
            .unwrap_or_default()
            .to_owned(),
        delivery: j2p::optional_pb_struct(obj, "delivery")?,
        supervisor: j2p::optional_pb_struct(obj, "supervisor")?,
    })
}

fn provenance_from_json(
    obj: &Map<String, Value>,
) -> Result<Option<pb::CeremonyInterventionProvenanceState>, String> {
    let Some(value) = obj.get("provenance") else {
        return Ok(None);
    };
    let provenance = j2p::require_object(value, "provenance")?;
    Ok(Some(pb::CeremonyInterventionProvenanceState {
        source_intervention_id: j2p::require_str(provenance, "source_intervention_id")?.to_owned(),
        source_response_role_id: j2p::require_str(provenance, "source_response_role_id")?
            .to_owned(),
        selected_role_id: j2p::require_str(provenance, "selected_role_id")?.to_owned(),
    }))
}

pub(super) fn build_respond_to_ceremony_intervention_request(
    args: &Value,
) -> Result<pb::RespondToCeremonyInterventionRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::RespondToCeremonyInterventionRequest {
        ceremony_id: j2p::require_str(obj, "ceremony_id")?.to_owned(),
        role_kind: j2p::require_str(obj, "role_kind")?.to_owned(),
        intervention_id: j2p::require_str(obj, "intervention_id")?.to_owned(),
        role_id: j2p::require_str(obj, "role_id")?.to_owned(),
        message: j2p::require_str(obj, "message")?.to_owned(),
        details: j2p::optional_pb_struct(obj, "details")?,
        delivery_id: j2p::optional_str(obj, "delivery_id")
            .unwrap_or_default()
            .to_owned(),
        agent_execution_id: j2p::optional_str(obj, "agent_execution_id")
            .unwrap_or_default()
            .to_owned(),
        incarnation: j2p::optional_str(obj, "incarnation")
            .unwrap_or_default()
            .to_owned(),
    })
}
