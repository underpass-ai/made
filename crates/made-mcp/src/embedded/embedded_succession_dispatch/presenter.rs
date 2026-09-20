use made_app::usecases::CeremonySuccessorPlanView;
use made_core::value_objects::{
    CarriedEvidence, ClaimDisposition, ClaimDispositionKind, DefinitionPin, SourceRecordRef,
    SuccessionPlan,
};
use serde_json::{json, Value};

pub(super) fn plan_view(view: &CeremonySuccessorPlanView) -> Value {
    json!({
        "predecessor_definition": pin(&view.predecessor_definition),
        "successor_definition": pin(&view.successor_definition),
        "diff": crate::embedded::present_definition_diff(&view.diff),
        "preflight": serde_json::to_value(&view.preflight).unwrap_or(Value::Null),
        "proposed_carried": view
            .proposed_carried
            .iter()
            .map(|proposed| json!({
                "step_id": proposed.step_id.as_str(),
                "source": source(&proposed.source),
            }))
            .collect::<Vec<_>>(),
        "required_dispositions": view
            .required_dispositions
            .iter()
            .map(|required| json!({
                "step_id": required.step_id.as_str(),
                "claim_fence": required.claim_fence.as_str(),
                "phase": required.phase,
            }))
            .collect::<Vec<_>>(),
        "strands": view
            .strands
            .iter()
            .map(|change| json!({
                "kind": change.kind().as_label(),
                "locus": serde_json::to_value(change.locus()).unwrap_or(Value::Null),
                "impact": change.impact().as_label(),
                "detail": change.detail(),
            }))
            .collect::<Vec<_>>(),
        "blockers": view
            .blockers
            .iter()
            .map(made_app::usecases::CeremonySuccessionBlocker::as_str)
            .collect::<Vec<_>>(),
        "ready": view.ready,
    })
}

pub(super) fn started(successor: &Value, plan: &SuccessionPlan) -> Value {
    json!({ "successor": successor, "plan": plan_value(plan) })
}

pub(crate) fn plan_value(plan: &SuccessionPlan) -> Value {
    json!({
        "plan_id": plan.plan_id().as_str(),
        "successor_id": plan.successor_id().as_str(),
        "successor_definition": pin(plan.successor_definition()),
        "carried": plan.carried().iter().map(carried).collect::<Vec<_>>(),
        "dispositions": plan.dispositions().iter().map(disposition).collect::<Vec<_>>(),
        "budget": plan.budget().as_label(),
        "planned_by": plan.planned_by().as_str(),
        "planned_at": plan
            .planned_at()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default(),
    })
}

fn carried(carried: &CarriedEvidence) -> Value {
    json!({
        "successor_step_id": carried.successor_step_id().as_str(),
        "source": source(carried.source()),
        "output": carried.output().attributes().as_map(),
    })
}

fn disposition(disposition: &ClaimDisposition) -> Value {
    let (evidence, receipt_id) = match disposition.kind() {
        ClaimDispositionKind::AbandonEffectReconciled { evidence } => {
            (Some(evidence.as_str()), None)
        }
        ClaimDispositionKind::CarryReceipt { receipt_id } => (None, Some(receipt_id.as_str())),
        ClaimDispositionKind::AbandonNoExternalEffect | ClaimDispositionKind::RetryInSuccessor => {
            (None, None)
        }
    };
    json!({
        "step_id": disposition.step_id().as_str(),
        "claim_fence": disposition.claim_fence().as_str(),
        "kind": disposition.kind().as_label(),
        "evidence": evidence,
        "receipt_id": receipt_id,
    })
}

pub(crate) fn source(source: &SourceRecordRef) -> Value {
    json!({
        "ceremony_id": source.ceremony_id().as_str(),
        "step_id": source.step_id().as_str(),
        "event_id": source.event_id().as_str(),
        "record_hash": source.record_hash().to_hex(),
        "state_visit": source.state_visit().get(),
        "attempt": source.attempt().get(),
    })
}

fn pin(pin: &DefinitionPin) -> Value {
    json!({
        "name": pin.name().as_str(),
        "version": pin.version().as_str(),
        "digest": pin.digest().to_hex(),
    })
}
