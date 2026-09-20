use crate::protocol::ToolError;
use made_app::usecases::{PlanCeremonySuccessorInput, StartCeremonySuccessorInput};
use made_core::value_objects::{
    Attributes, AuditActorId, BudgetDisposition, CeremonyContext, CeremonyId, CeremonyName,
    CeremonyVersion, ClaimDisposition, ClaimDispositionKind, EvidenceReference, ExecutionReceiptId,
    IdempotencyKey, StepClaimFence, StepId,
};
use serde_json::Value;

pub(super) fn plan(arguments: &Value) -> Result<PlanCeremonySuccessorInput, ToolError> {
    Ok(PlanCeremonySuccessorInput {
        instance_id: CeremonyId::new(required(arguments, "ceremony_id")?)?,
        definition_name: CeremonyName::new(required(arguments, "definition_name")?)?,
        definition_version: CeremonyVersion::new(required(arguments, "definition_version")?)?,
    })
}

pub(super) fn start(arguments: &Value) -> Result<StartCeremonySuccessorInput, ToolError> {
    let carried = arguments
        .get("carried")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| StepId::new(item.as_str().unwrap_or_default()).map_err(ToolError::from))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    let dispositions = arguments
        .get("dispositions")
        .and_then(Value::as_array)
        .map(|items| items.iter().map(disposition).collect::<Result<Vec<_>, _>>())
        .transpose()?
        .unwrap_or_default();
    let context_overrides = arguments
        .get("context_overrides")
        .filter(|value| !value.is_null())
        .map(|value| {
            serde_json::from_value::<Attributes>(value.clone())
                .map_err(|error| ToolError::invalid_request(error.to_string()))
                .map(CeremonyContext::new)
        })
        .transpose()?;
    Ok(StartCeremonySuccessorInput {
        instance_id: CeremonyId::new(required(arguments, "ceremony_id")?)?,
        plan_id: IdempotencyKey::new(required(arguments, "plan_id")?)?,
        definition_name: CeremonyName::new(required(arguments, "definition_name")?)?,
        definition_version: CeremonyVersion::new(required(arguments, "definition_version")?)?,
        carried,
        dispositions,
        budget: budget(arguments)?,
        context_overrides,
        actor_id: AuditActorId::new(required(arguments, "actor_id")?),
        actor_kind: serde_json::from_value(Value::String(
            required(arguments, "actor_kind")?.to_owned(),
        ))
        .map_err(|error| ToolError::invalid_request(error.to_string()))?,
    })
}

/// Only `fresh` and the omitted default are honoured here.
/// `transfer_remaining` is accepted as a spelling so the refusal comes
/// from the decision, with its reason, rather than from a parser.
fn budget(arguments: &Value) -> Result<BudgetDisposition, ToolError> {
    match arguments.get("budget").and_then(Value::as_str) {
        None | Some("fresh") => Ok(BudgetDisposition::Fresh),
        Some("transfer_remaining") => Ok(BudgetDisposition::TransferRemaining),
        Some(other) => Err(ToolError::invalid_request(format!(
            "unknown budget disposition `{other}`"
        ))),
    }
}

fn disposition(value: &Value) -> Result<ClaimDisposition, ToolError> {
    let kind = match required(value, "kind")? {
        "abandon_no_external_effect" => ClaimDispositionKind::AbandonNoExternalEffect,
        "abandon_effect_reconciled" => ClaimDispositionKind::AbandonEffectReconciled {
            evidence: EvidenceReference::new(required(value, "evidence")?)?,
        },
        "carry_receipt" => ClaimDispositionKind::CarryReceipt {
            receipt_id: ExecutionReceiptId::new(required(value, "receipt_id")?)?,
        },
        "retry_in_successor" => ClaimDispositionKind::RetryInSuccessor,
        other => {
            return Err(ToolError::invalid_request(format!(
                "unknown claim disposition `{other}`"
            )))
        }
    };
    Ok(ClaimDisposition::new(
        StepId::new(required(value, "step_id")?)?,
        StepClaimFence::new(required(value, "claim_fence")?)?,
        kind,
    ))
}

fn required<'a>(value: &'a Value, field: &str) -> Result<&'a str, ToolError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::invalid_request(format!("missing required field `{field}`")))
}
