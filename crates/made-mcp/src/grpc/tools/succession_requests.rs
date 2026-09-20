use super::super::json_to_proto as j2p;
use made_mcp_proto::v1 as pb;
use serde_json::Value;

pub(super) fn plan(arguments: &Value) -> Result<pb::PlanCeremonySuccessorRequest, String> {
    let obj = j2p::require_object(arguments, "arguments")?;
    Ok(pb::PlanCeremonySuccessorRequest {
        ceremony_id: j2p::require_str(obj, "ceremony_id")?.into(),
        definition_name: j2p::require_str(obj, "definition_name")?.into(),
        definition_version: j2p::require_str(obj, "definition_version")?.into(),
    })
}

pub(super) fn start(arguments: &Value) -> Result<pb::StartCeremonySuccessorRequest, String> {
    let obj = j2p::require_object(arguments, "arguments")?;
    let carried = obj
        .get("carried")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    item.as_str()
                        .map(ToOwned::to_owned)
                        .ok_or_else(|| "carried holds step ids".to_owned())
                })
                .collect::<Result<Vec<_>, String>>()
        })
        .transpose()?
        .unwrap_or_default();
    let dispositions = obj
        .get("dispositions")
        .and_then(Value::as_array)
        .map(|items| items.iter().map(disposition).collect::<Result<Vec<_>, _>>())
        .transpose()?
        .unwrap_or_default();
    Ok(pb::StartCeremonySuccessorRequest {
        ceremony_id: j2p::require_str(obj, "ceremony_id")?.into(),
        plan_id: j2p::require_str(obj, "plan_id")?.into(),
        definition_name: j2p::require_str(obj, "definition_name")?.into(),
        definition_version: j2p::require_str(obj, "definition_version")?.into(),
        carried,
        dispositions,
        budget: j2p::optional_str(obj, "budget").unwrap_or_default().into(),
        context_overrides: j2p::optional_pb_struct(obj, "context_overrides")?,
        actor_id: j2p::require_str(obj, "actor_id")?.into(),
        actor_kind: j2p::require_str(obj, "actor_kind")?.into(),
    })
}

fn disposition(value: &Value) -> Result<pb::CeremonyClaimDisposition, String> {
    let obj = j2p::require_object(value, "dispositions[]")?;
    Ok(pb::CeremonyClaimDisposition {
        step_id: j2p::require_str(obj, "step_id")?.into(),
        claim_fence: j2p::require_str(obj, "claim_fence")?.into(),
        kind: j2p::require_str(obj, "kind")?.into(),
        evidence: j2p::optional_str(obj, "evidence")
            .unwrap_or_default()
            .into(),
        receipt_id: j2p::optional_str(obj, "receipt_id")
            .unwrap_or_default()
            .into(),
    })
}
