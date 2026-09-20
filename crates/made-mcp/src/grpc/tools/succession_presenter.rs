use made_mcp_proto::v1 as pb;
use serde_json::{json, Value};

use crate::grpc::proto_to_json::{
    ceremony_instance_state_to_json, diff_ceremony_definitions_to_json, nullable_pb_struct_to_json,
    optional_pb_struct_to_json,
};

pub(super) fn plan_view(view: pb::CeremonySuccessorPlanView) -> Value {
    json!({
        "predecessor_definition": view.predecessor_definition.as_ref().map(pin),
        "successor_definition": view.successor_definition.as_ref().map(pin),
        "diff": view.diff.map(diff_ceremony_definitions_to_json),
        "preflight": view.preflight.map(super::host_handoff_presenter::preflight),
        "proposed_carried": view
            .proposed_carried
            .into_iter()
            .map(|proposed| json!({
                "step_id": proposed.step_id,
                "source": proposed.source.as_ref().map(source),
            }))
            .collect::<Vec<_>>(),
        "required_dispositions": view
            .required_dispositions
            .into_iter()
            .map(|required| json!({
                "step_id": required.step_id,
                "claim_fence": required.claim_fence,
                "phase": required.phase,
            }))
            .collect::<Vec<_>>(),
        "strands": view.strands.into_iter().map(change).collect::<Vec<_>>(),
        "blockers": view.blockers,
        "ready": view.ready,
    })
}

pub(super) fn started(response: pb::StartCeremonySuccessorResponse) -> Value {
    json!({
        "successor": response
            .successor
            .map(ceremony_instance_state_to_json),
        "plan": response.plan.map(plan),
    })
}

fn plan(plan: pb::CeremonySuccessionPlan) -> Value {
    json!({
        "plan_id": plan.plan_id,
        "successor_id": plan.successor_id,
        "successor_definition": plan.successor_definition.as_ref().map(pin),
        "carried": plan
            .carried
            .into_iter()
            .map(|carried| json!({
                "successor_step_id": carried.successor_step_id,
                "source": carried.source.as_ref().map(source),
                "output": optional_pb_struct_to_json(carried.output.clone()),
            }))
            .collect::<Vec<_>>(),
        "dispositions": plan
            .dispositions
            .into_iter()
            .map(disposition)
            .collect::<Vec<_>>(),
        "budget": plan.budget,
        "planned_by": plan.planned_by,
        "planned_at": plan.planned_at,
    })
}

fn disposition(disposition: pb::CeremonyClaimDisposition) -> Value {
    json!({
        "step_id": disposition.step_id,
        "claim_fence": disposition.claim_fence,
        "kind": disposition.kind,
        "evidence": optional(disposition.evidence),
        "receipt_id": optional(disposition.receipt_id),
    })
}

fn change(change: pb::CeremonyDefinitionChange) -> Value {
    json!({
        "kind": change.kind,
        "locus": nullable_pb_struct_to_json(change.locus),
        "impact": change.impact,
        "detail": change.detail,
    })
}

fn source(source: &pb::CeremonySourceRecordRef) -> Value {
    json!({
        "ceremony_id": source.ceremony_id,
        "step_id": source.step_id,
        "event_id": source.event_id,
        "record_hash": source.record_hash,
        "state_visit": source.state_visit,
        "attempt": source.attempt,
    })
}

fn pin(pin: &pb::CeremonyDefinitionPin) -> Value {
    json!({ "name": pin.name, "version": pin.version, "digest": pin.digest })
}

fn optional(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}
