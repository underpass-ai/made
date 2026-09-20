use made_mcp_proto::v1 as pb;
use serde_json::{json, Value};

use crate::grpc::proto_to_json::{
    diff_ceremony_definitions_to_json, nullable_pb_struct_to_json, source_record_ref_to_json,
    succession_plan_to_json,
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
                "source": proposed.source.as_ref().map(source_record_ref_to_json),
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
        "successor_id": response.successor_id,
        "plan": response.plan.map(succession_plan_to_json),
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

fn pin(pin: &pb::CeremonyDefinitionPin) -> Value {
    json!({ "name": pin.name, "version": pin.version, "digest": pin.digest })
}
