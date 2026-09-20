//! Succession, as the two MCP backends answer it.
//!
//! Rendered from the contract's own messages so the gRPC backend
//! answers the shape the in-process one answers. A shape one backend
//! has and the other does not is exactly what the parity gate refuses.

use made_mcp_proto::v1 as pb;
use serde_json::{json, Value};

use super::primitives::optional_pb_struct_to_json;

/// Proto3 says "absent" with an empty string; JSON says it with null.
fn empty_as_null(value: String) -> Value {
    if value.is_empty() {
        Value::Null
    } else {
        Value::String(value)
    }
}

/// Which ceremony this one succeeds, when it succeeds one.
pub(super) fn succession_to_json(succession: &pb::CeremonySuccessionState) -> Value {
    json!({
        "predecessor_id": succession.predecessor_id,
        "predecessor_head": succession.predecessor_head,
        "predecessor_version": succession.predecessor_version,
        "predecessor_definition": succession.predecessor_definition.as_ref().map(pin_to_json),
        "successor_definition": succession.successor_definition.as_ref().map(pin_to_json),
        "plan_id": succession.plan_id,
    })
}

/// The handoff this ceremony sealed, when it sealed one.
pub(crate) fn succession_plan_to_json(plan: pb::CeremonySuccessionPlan) -> Value {
    json!({
        "plan_id": plan.plan_id,
        "successor_id": plan.successor_id,
        "successor_definition": plan.successor_definition.as_ref().map(pin_to_json),
        "carried": plan
            .carried
            .into_iter()
            .map(|carried| json!({
                "successor_step_id": carried.successor_step_id,
                "source": carried.source.as_ref().map(source_record_ref_to_json),
                "output": optional_pb_struct_to_json(carried.output),
            }))
            .collect::<Vec<_>>(),
        "dispositions": plan
            .dispositions
            .into_iter()
            .map(|disposition| json!({
                "step_id": disposition.step_id,
                "claim_fence": disposition.claim_fence,
                "kind": disposition.kind,
                "evidence": empty_as_null(disposition.evidence),
                "receipt_id": empty_as_null(disposition.receipt_id),
            }))
            .collect::<Vec<_>>(),
        "budget": plan.budget,
        "planned_by": plan.planned_by,
        "planned_at": plan.planned_at,
    })
}

pub(crate) fn source_record_ref_to_json(source: &pb::CeremonySourceRecordRef) -> Value {
    json!({
        "ceremony_id": source.ceremony_id,
        "step_id": source.step_id,
        "event_id": source.event_id,
        "record_hash": source.record_hash,
        "state_visit": source.state_visit,
        "attempt": source.attempt,
    })
}

fn pin_to_json(pin: &pb::CeremonyDefinitionPin) -> Value {
    json!({ "name": pin.name, "version": pin.version, "digest": pin.digest })
}
