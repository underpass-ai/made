use made_mcp_proto::v1 as pb;
use serde_json::Value;

use super::super::json_to_proto as j2p;

pub(super) fn bind(arguments: &Value) -> Result<pb::BindCeremonyIntegratorRequest, String> {
    let obj = j2p::require_object(arguments, "arguments")?;
    Ok(pb::BindCeremonyIntegratorRequest {
        binding_id: j2p::require_str(obj, "binding_id")?.into(),
        scope: Some(scope(obj)?),
        role_id: j2p::require_str(obj, "role_id")?.into(),
        host_kind: j2p::require_str(obj, "host_kind")?.into(),
        address: j2p::require_str(obj, "address")?.into(),
        activation: j2p::optional_str(obj, "activation")
            .unwrap_or_default()
            .into(),
        incarnation: j2p::require_str(obj, "incarnation")?.into(),
        replace: j2p::optional_bool(obj, "replace"),
        follow_replacement: j2p::optional_bool(obj, "follow_replacement"),
    })
}

pub(super) fn binding(
    arguments: &Value,
) -> Result<pb::GetCeremonyIntegratorBindingRequest, String> {
    let obj = j2p::require_object(arguments, "arguments")?;
    Ok(pb::GetCeremonyIntegratorBindingRequest {
        scope: Some(scope(obj)?),
    })
}

pub(super) fn await_attention(
    arguments: &Value,
) -> Result<pb::AwaitIntegratorAttentionRequest, String> {
    let obj = j2p::require_object(arguments, "arguments")?;
    Ok(pb::AwaitIntegratorAttentionRequest {
        scope: Some(scope(obj)?),
        binding_id: j2p::require_str(obj, "binding_id")?.into(),
        incarnation: j2p::require_str(obj, "incarnation")?.into(),
        fence: fence(obj)?,
        limit: j2p::optional_u32(obj, "limit")?,
        wait_timeout_ms: j2p::optional_u64(obj, "wait_timeout_ms")?,
        lease_duration_ms: j2p::optional_u64(obj, "lease_duration_ms")?,
    })
}

pub(super) fn acknowledge(
    arguments: &Value,
) -> Result<pb::AcknowledgeIntegratorAttentionRequest, String> {
    let obj = j2p::require_object(arguments, "arguments")?;
    Ok(pb::AcknowledgeIntegratorAttentionRequest {
        binding_id: j2p::require_str(obj, "binding_id")?.into(),
        incarnation: j2p::require_str(obj, "incarnation")?.into(),
        fence: fence(obj)?,
        delivery_id: j2p::require_str(obj, "delivery_id")?.into(),
        lease_id: j2p::require_str(obj, "lease_id")?.into(),
        acknowledgement: j2p::require_str(obj, "acknowledgement")?.into(),
        action_kind: j2p::optional_str(obj, "action_kind")
            .unwrap_or_default()
            .into(),
        idempotency_key: j2p::optional_str(obj, "idempotency_key")
            .unwrap_or_default()
            .into(),
        note: j2p::optional_str(obj, "note").unwrap_or_default().into(),
        evidence: j2p::optional_str(obj, "evidence")
            .unwrap_or_default()
            .into(),
        failure_reason: j2p::optional_str(obj, "failure_reason")
            .unwrap_or_default()
            .into(),
    })
}

pub(super) fn deliveries(arguments: &Value) -> Result<pb::ListAttentionDeliveriesRequest, String> {
    let obj = j2p::require_object(arguments, "arguments")?;
    Ok(pb::ListAttentionDeliveriesRequest {
        binding_id: j2p::optional_str(obj, "binding_id")
            .unwrap_or_default()
            .into(),
        ceremony_id: j2p::optional_str(obj, "ceremony_id")
            .unwrap_or_default()
            .into(),
        state: j2p::optional_str(obj, "state").unwrap_or_default().into(),
        limit: j2p::optional_u32(obj, "limit")?,
        cursor: j2p::optional_str(obj, "cursor").unwrap_or_default().into(),
    })
}

/// The scope, which is a nested object rather than a pair of loose ids
/// so that a caller cannot name a ceremony and a system run at once.
fn scope(obj: &serde_json::Map<String, Value>) -> Result<pb::IntegratorScopeState, String> {
    let scope = obj
        .get("scope")
        .ok_or_else(|| "missing required string `scope`".to_owned())?;
    let scope = j2p::require_object(scope, "scope")?;
    Ok(pb::IntegratorScopeState {
        kind: j2p::require_str(scope, "kind")?.into(),
        ceremony_id: j2p::optional_str(scope, "ceremony_id")
            .unwrap_or_default()
            .into(),
        system_execution_id: j2p::optional_str(scope, "system_execution_id")
            .unwrap_or_default()
            .into(),
    })
}

/// Required rather than defaulted: the first fence is zero, and a
/// caller that omitted it would be presenting the first generation of
/// a binding it may have been displaced from.
fn fence(obj: &serde_json::Map<String, Value>) -> Result<u64, String> {
    obj.get("fence")
        .ok_or_else(|| "missing required integer `fence`".to_owned())?
        .as_u64()
        .ok_or_else(|| "invalid fence".to_owned())
}
