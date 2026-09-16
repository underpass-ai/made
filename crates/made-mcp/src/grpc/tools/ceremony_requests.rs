use made_mcp_proto::v1 as pb;
use serde_json::{Map, Value};
use uuid::Uuid;

use super::super::json_to_proto as j2p;
use crate::grpc::GRPC_BACKEND_NAME;
use crate::protocol::{
    default_idempotency_key, default_lease_owner_id, CLAIM_CEREMONY_STEP_LEASE_TTL_MS,
    RUN_CEREMONY_STEP_LEASE_TTL_MS,
};

/// The runner the caller named, or the one this layer applies.
///
/// Absent means "you choose"; blank is refused, as the tool's schema
/// says (`minLength: 1`) and as the in-process backend already did.
/// Letting it through would reach the server's own default, which is
/// the divergence this closes.
pub(in crate::grpc) fn lease_owner_id(obj: &Map<String, Value>) -> Result<String, String> {
    match j2p::optional_str(obj, "lease_owner_id") {
        None => Ok(default_lease_owner_id(GRPC_BACKEND_NAME)),
        Some(named) if named.trim().is_empty() => {
            Err("field `lease_owner_id` must not be blank".to_owned())
        }
        Some(named) => Ok(named.trim().to_owned()),
    }
}

/// The execution key the caller named, or the one this layer mints.
///
/// Minted here rather than left empty for the server to mint: the key
/// is sealed into the journal, and a journal that records
/// `grpc-claim-<uuid>` for the same call the in-process engine records
/// `made-mcp-external-<uuid>` for tells a reader which process answered
/// instead of what the client asked.
pub(in crate::grpc) fn idempotency_key(obj: &Map<String, Value>) -> String {
    j2p::optional_str(obj, "idempotency_key")
        .map(str::trim)
        .filter(|named| !named.is_empty())
        .map_or_else(default_idempotency_key, ToOwned::to_owned)
}

/// The lease length the caller asked for, or the one this layer
/// applies.
///
/// Sent rather than left as a zero: `0` on the wire means "you choose",
/// and letting the server choose is how the same omission became a
/// sixty-second lease over the wire and a thirty-second one in process.
pub(in crate::grpc) fn lease_ttl_ms(
    obj: &Map<String, Value>,
    default_ms: u64,
) -> Result<u64, String> {
    Ok(match j2p::optional_u64(obj, "lease_ttl_ms")? {
        0 => default_ms,
        asked => asked,
    })
}

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
        // Defaulted here, not left to the server: one omission, one
        // owner, whichever backend the client is pointed at.
        lease_owner_id: lease_owner_id(obj)?,
        idempotency_key: idempotency_key(obj),
        lease_ttl_ms: lease_ttl_ms(obj, RUN_CEREMONY_STEP_LEASE_TTL_MS)?,
    })
}

/// The two ends of a step the host runs itself. Same arguments as
/// the in-process tools take, because it is the same tool: only the
/// engine on the other side of it changes.
pub(super) fn build_claim_ceremony_step_request(
    args: &Value,
) -> Result<pb::ClaimCeremonyStepRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::ClaimCeremonyStepRequest {
        ceremony_id: j2p::require_str(obj, "ceremony_id")?.to_owned(),
        step_id: j2p::require_str(obj, "step_id")?.to_owned(),
        actor_kind: j2p::require_str(obj, "actor_kind")?.to_owned(),
        // Defaulted here, exactly as `made_run_ceremony_step` does:
        // one omission, one owner, whichever backend the client is
        // pointed at. The server keeps its own default for clients
        // that speak gRPC directly; from MCP it is never reached.
        lease_owner_id: lease_owner_id(obj)?,
        idempotency_key: idempotency_key(obj),
        lease_ttl_ms: lease_ttl_ms(obj, CLAIM_CEREMONY_STEP_LEASE_TTL_MS)?,
    })
}

pub(super) fn build_complete_ceremony_step_request(
    args: &Value,
) -> Result<pb::CompleteCeremonyStepRequest, String> {
    let obj = j2p::require_object(args, "tools/call.arguments")?;
    Ok(pb::CompleteCeremonyStepRequest {
        ceremony_id: j2p::require_str(obj, "ceremony_id")?.to_owned(),
        step_id: j2p::require_str(obj, "step_id")?.to_owned(),
        actor_kind: j2p::require_str(obj, "actor_kind")?.to_owned(),
        status: j2p::require_str(obj, "status")?.to_owned(),
        output: j2p::optional_pb_struct(obj, "output")?,
        // Empty is absent on the wire, and the server refuses a
        // failure that carries no reason.
        error: j2p::optional_str(obj, "error")
            .unwrap_or_default()
            .to_owned(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The request that leaves this layer already names a runner, so
    /// the server's own default is never reached from MCP.
    #[test]
    fn an_omitted_runner_becomes_the_backends_own_default() {
        let request = build_run_ceremony_step_request(&json!({
            "ceremony_id": "c-1",
            "step_id": "work",
            "actor_kind": "agent",
        }))
        .expect("the request should be accepted");
        assert_eq!(request.lease_owner_id, "made-mcp:grpc");
    }

    /// Blank is refused rather than defaulted, as the tool's schema
    /// says and as the in-process backend already did: a caller who
    /// wrote the field meant to name a runner.
    #[test]
    fn a_blank_runner_is_refused_rather_than_defaulted() {
        let error = build_run_ceremony_step_request(&json!({
            "ceremony_id": "c-1",
            "step_id": "work",
            "actor_kind": "agent",
            "lease_owner_id": "   ",
        }))
        .expect_err("a blank runner is not a runner");
        assert!(error.contains("lease_owner_id"), "{error}");
    }

    /// The claim follows the same rule: it is the same lease, taken by
    /// the same MCP server, so an omission cannot mean one owner when
    /// the engine runs the step and another when the host does.
    #[test]
    fn a_claim_applies_the_same_default_runner() {
        let request = build_claim_ceremony_step_request(&json!({
            "ceremony_id": "c-1",
            "step_id": "work",
            "actor_kind": "agent",
        }))
        .expect("the request should be accepted");
        assert_eq!(request.lease_owner_id, "made-mcp:grpc");

        let error = build_claim_ceremony_step_request(&json!({
            "ceremony_id": "c-1",
            "step_id": "work",
            "actor_kind": "agent",
            "lease_owner_id": " ",
        }))
        .expect_err("a blank runner is not a runner");
        assert!(error.contains("lease_owner_id"), "{error}");

        let request = build_claim_ceremony_step_request(&json!({
            "ceremony_id": "c-1",
            "step_id": "work",
            "actor_kind": "agent",
            "lease_owner_id": "the-hosts-own-runner",
        }))
        .expect("the request should be accepted");
        assert_eq!(request.lease_owner_id, "the-hosts-own-runner");
    }

    /// The key leaves this layer minted, so the server's own — sealed
    /// into the journal with a transport name in it — is never reached.
    #[test]
    fn an_omitted_execution_key_is_minted_by_this_layer() {
        let step = build_run_ceremony_step_request(&json!({
            "ceremony_id": "c-1",
            "step_id": "work",
            "actor_kind": "agent",
        }))
        .expect("the request should be accepted");
        assert!(
            step.idempotency_key.starts_with("made-mcp:"),
            "{}",
            step.idempotency_key
        );

        let claim = build_claim_ceremony_step_request(&json!({
            "ceremony_id": "c-1",
            "step_id": "work",
            "actor_kind": "agent",
        }))
        .expect("the request should be accepted");
        assert!(
            claim.idempotency_key.starts_with("made-mcp:"),
            "{}",
            claim.idempotency_key
        );
        assert_ne!(
            step.idempotency_key, claim.idempotency_key,
            "two calls are two executions"
        );
    }

    #[test]
    fn an_execution_key_the_caller_named_is_left_alone() {
        let step = build_run_ceremony_step_request(&json!({
            "ceremony_id": "c-1",
            "step_id": "work",
            "actor_kind": "agent",
            "idempotency_key": "retry-42",
        }))
        .expect("the request should be accepted");
        assert_eq!(step.idempotency_key, "retry-42");
    }

    /// The lease length leaves this layer as a number, so the server's
    /// own default is never reached — the divergence that made one
    /// omission mean two different leases.
    #[test]
    fn an_omitted_lease_length_becomes_the_number_this_layer_sends() {
        let step = build_run_ceremony_step_request(&json!({
            "ceremony_id": "c-1",
            "step_id": "work",
            "actor_kind": "agent",
        }))
        .expect("the request should be accepted");
        assert_eq!(step.lease_ttl_ms, RUN_CEREMONY_STEP_LEASE_TTL_MS);

        let claim = build_claim_ceremony_step_request(&json!({
            "ceremony_id": "c-1",
            "step_id": "work",
            "actor_kind": "agent",
        }))
        .expect("the request should be accepted");
        assert_eq!(claim.lease_ttl_ms, CLAIM_CEREMONY_STEP_LEASE_TTL_MS);
    }

    /// Zero is the schema's own spelling of "omitted", so it takes the
    /// same answer rather than reaching the server's default by the
    /// back door.
    #[test]
    fn a_zero_lease_length_reads_as_an_omission() {
        let step = build_run_ceremony_step_request(&json!({
            "ceremony_id": "c-1",
            "step_id": "work",
            "actor_kind": "agent",
            "lease_ttl_ms": 0,
        }))
        .expect("the request should be accepted");
        assert_eq!(step.lease_ttl_ms, RUN_CEREMONY_STEP_LEASE_TTL_MS);
    }

    #[test]
    fn a_lease_length_the_caller_asked_for_is_left_alone() {
        let step = build_run_ceremony_step_request(&json!({
            "ceremony_id": "c-1",
            "step_id": "work",
            "actor_kind": "agent",
            "lease_ttl_ms": 5_000,
        }))
        .expect("the request should be accepted");
        assert_eq!(step.lease_ttl_ms, 5_000);
    }

    #[test]
    fn a_runner_the_caller_named_is_left_alone() {
        let request = build_run_ceremony_step_request(&json!({
            "ceremony_id": "c-1",
            "step_id": "work",
            "actor_kind": "agent",
            "lease_owner_id": "the-hosts-own-runner",
        }))
        .expect("the request should be accepted");
        assert_eq!(request.lease_owner_id, "the-hosts-own-runner");
    }
}
