use made_mcp_proto::v1 as pb;
use serde_json::{Map, Value};
use uuid::Uuid;

use super::super::json_to_proto as j2p;

pub(super) fn definition_yaml(args: &Value) -> Result<String, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(j2p::require_str(obj, "definition_yaml")?.to_owned())
}

/// Minting an id client-side when the caller left it out, exactly as
/// the in-process backend does: a tool that demands an identifier for
/// a thing that does not exist yet is a tool that makes its caller
/// invent one.
fn minted_id(obj: &Map<String, Value>, key: &str) -> String {
    j2p::optional_str(obj, key)
        .filter(|value| !value.trim().is_empty())
        .map_or_else(|| Uuid::new_v4().to_string(), ToOwned::to_owned)
}

pub(super) fn build_start_ceremony_request(
    args: &Value,
) -> Result<pb::StartCeremonyRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::StartCeremonyRequest {
        actor_id: j2p::require_str(obj, "actor_id")?.to_owned(),
        actor_kind: j2p::require_str(obj, "actor_kind")?.to_owned(),
        ceremony_id: minted_id(obj, "ceremony_id"),
        definition_yaml: j2p::require_str(obj, "definition_yaml")?.to_owned(),
        context: j2p::optional_pb_struct(obj, "context")?,
    })
}

pub(super) fn build_start_published_ceremony_request(
    args: &Value,
) -> Result<pb::StartPublishedCeremonyRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::StartPublishedCeremonyRequest {
        actor_id: j2p::require_str(obj, "actor_id")?.to_owned(),
        actor_kind: j2p::require_str(obj, "actor_kind")?.to_owned(),
        ceremony_id: minted_id(obj, "ceremony_id"),
        ceremony: j2p::require_str(obj, "ceremony")?.to_owned(),
        version: j2p::require_str(obj, "version")?.to_owned(),
        context: j2p::optional_pb_struct(obj, "context")?,
    })
}

pub(super) fn build_run_ceremony_step_request(
    args: &Value,
) -> Result<pb::RunCeremonyStepRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::RunCeremonyStepRequest {
        ceremony_id: j2p::require_str(obj, "ceremony_id")?.to_owned(),
        actor_kind: j2p::require_str(obj, "actor_kind")?.to_owned(),
        step_id: j2p::require_str(obj, "step_id")?.to_owned(),
        lease_owner_id: j2p::optional_str(obj, "lease_owner_id")
            .unwrap_or_default()
            .to_owned(),
        idempotency_key: j2p::optional_str(obj, "idempotency_key")
            .unwrap_or_default()
            .to_owned(),
        lease_ttl_ms: j2p::optional_u64(obj, "lease_ttl_ms")?,
    })
}

pub(super) fn build_apply_ceremony_transition_request(
    args: &Value,
) -> Result<pb::ApplyCeremonyTransitionRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::ApplyCeremonyTransitionRequest {
        ceremony_id: j2p::require_str(obj, "ceremony_id")?.to_owned(),
        trigger: j2p::require_str(obj, "trigger")?.to_owned(),
        actor_kind: j2p::require_str(obj, "actor_kind")?.to_owned(),
    })
}

pub(super) fn build_approve_ceremony_guard_request(
    args: &Value,
) -> Result<pb::ApproveCeremonyGuardRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::ApproveCeremonyGuardRequest {
        ceremony_id: j2p::require_str(obj, "ceremony_id")?.to_owned(),
        guard_name: j2p::require_str(obj, "guard_name")?.to_owned(),
        role_id: j2p::require_str(obj, "role_id")?.to_owned(),
        role_kind: j2p::require_str(obj, "role_kind")?.to_owned(),
    })
}

/// One end of a reason, from the tool's JSON.
///
/// Only the field the kind names is read, which is what the wire does
/// too — the discriminator is what the object means.
pub(super) fn build_ceremony_record_ref(
    value: &Value,
    field: &str,
) -> Result<pb::CeremonyRecordRefState, String> {
    let obj = j2p::require_object(value, field)?;
    Ok(pb::CeremonyRecordRefState {
        kind: j2p::require_str(obj, "kind")?.to_owned(),
        step_id: obj
            .get("step_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        agenda_item: obj
            .get("agenda_item")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        ordinal: obj
            .get("ordinal")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or_default(),
        guard_name: obj
            .get("guard_name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
    })
}

pub(super) fn build_assert_ceremony_reason_request(
    args: &Value,
) -> Result<pb::AssertCeremonyReasonRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::AssertCeremonyReasonRequest {
        ceremony_id: j2p::require_str(obj, "ceremony_id")?.to_owned(),
        role_kind: j2p::require_str(obj, "role_kind")?.to_owned(),
        role_id: j2p::require_str(obj, "role_id")?.to_owned(),
        from: Some(build_ceremony_record_ref(
            obj.get("from").ok_or("`from` is required")?,
            "from",
        )?),
        to: Some(build_ceremony_record_ref(
            obj.get("to").ok_or("`to` is required")?,
            "to",
        )?),
        kind: j2p::require_str(obj, "kind")?.to_owned(),
        why: j2p::require_str(obj, "why")?.to_owned(),
        confidence: j2p::require_str(obj, "confidence")?.to_owned(),
    })
}

pub(super) fn build_defer_ceremony_guard_request(
    args: &Value,
) -> Result<pb::DeferCeremonyGuardRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::DeferCeremonyGuardRequest {
        ceremony_id: j2p::require_str(obj, "ceremony_id")?.to_owned(),
        guard_name: j2p::require_str(obj, "guard_name")?.to_owned(),
        role_id: j2p::require_str(obj, "role_id")?.to_owned(),
        role_kind: j2p::require_str(obj, "role_kind")?.to_owned(),
        statement: j2p::require_str(obj, "statement")?.to_owned(),
        reason: j2p::require_str(obj, "reason")?.to_owned(),
        reconsider_when: j2p::string_array(obj, "reconsider_when"),
    })
}

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
    })
}

pub(super) fn build_close_ceremony_intervention_request(
    args: &Value,
) -> Result<pb::CloseCeremonyInterventionRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::CloseCeremonyInterventionRequest {
        ceremony_id: j2p::require_str(obj, "ceremony_id")?.to_owned(),
        role_kind: j2p::require_str(obj, "role_kind")?.to_owned(),
        intervention_id: j2p::require_str(obj, "intervention_id")?.to_owned(),
        role_id: j2p::require_str(obj, "role_id")?.to_owned(),
    })
}

pub(super) fn build_collect_ceremony_evidence_request(
    args: &Value,
) -> Result<pb::CollectCeremonyEvidenceRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::CollectCeremonyEvidenceRequest {
        ceremony_id: j2p::require_str(obj, "ceremony_id")?.to_owned(),
        role_kind: j2p::require_str(obj, "role_kind")?.to_owned(),
        intervention_id: j2p::require_str(obj, "intervention_id")?.to_owned(),
        role_id: j2p::require_str(obj, "role_id")?.to_owned(),
        source_id: j2p::require_str(obj, "source_id")?.to_owned(),
        query: j2p::require_str(obj, "query")?.to_owned(),
        details: j2p::optional_pb_struct(obj, "details")?,
    })
}

/// One side of a comparison. Absent is an error here rather than a
/// default: there is no sensible definition to compare against when
/// the caller named none.
pub(super) fn definition_ref(
    obj: &Map<String, Value>,
    key: &str,
) -> Result<Option<pb::CeremonyDefinitionRef>, String> {
    let value = obj
        .get(key)
        .ok_or_else(|| format!("missing required object `{key}`"))?;
    let reference = j2p::require_object(value, key)?;
    Ok(Some(pb::CeremonyDefinitionRef {
        ceremony: j2p::optional_str(reference, "ceremony")
            .unwrap_or_default()
            .to_owned(),
        version: j2p::optional_str(reference, "version")
            .unwrap_or_default()
            .to_owned(),
        definition_yaml: j2p::optional_str(reference, "definition_yaml")
            .unwrap_or_default()
            .to_owned(),
    }))
}
