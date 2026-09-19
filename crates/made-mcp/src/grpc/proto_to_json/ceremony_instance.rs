//! A working session, from proto to the MCP contract's JSON.
//!
//! Split out of `proto_to_json` because the session is the part of the
//! contract both backends must render identically, and it is the part
//! that keeps growing.

use made_mcp_proto::v1 as pb;
use serde_json::{json, Value};

use super::optional_pb_struct_to_json;
use crate::renderers::CeremonyInstanceListingEntry;

/// Return the fence captured by the claim, alongside that accepted instance.
pub(crate) fn ceremony_claim_to_json(response: pb::ClaimCeremonyStepResponse) -> Option<Value> {
    response.instance.map(|instance| {
        let mut value = ceremony_instance_state_to_json(instance);
        value["claim_fence"] = json!(response.claim_fence);
        value["budget"] = response
            .budget
            .as_ref()
            .map_or(Value::Null, super::budget_admission_to_json);
        value
    })
}

/// A live working session as the MCP contract carries it.
///
/// This is the shape the in-process backend renders from the domain,
/// reproduced from the proto. Proto has no null, so absence arrives as
/// an empty string and is put back as `null` here: a client must not
/// have to know which backend answered in order to tell "no next step"
/// from "the next step is called nothing".
/// One entry of `made_list_ceremony_instances`.
///
/// Every entry says whether it could be read, so a caller tests one
/// field instead of inferring readability from a field that is not
/// there. An entry that could not be read carries its id and the
/// reason and nothing else: there is nothing else to carry.
pub(crate) fn ceremony_instance_listing_entry(
    state: pb::CeremonyInstanceState,
) -> CeremonyInstanceListingEntry {
    if !state.rehydratable {
        return CeremonyInstanceListingEntry::unrehydratable(
            state.ceremony_id,
            state.unrehydratable_reason,
        );
    }
    CeremonyInstanceListingEntry::rehydratable(ceremony_instance_state_to_json(state))
}

pub(crate) fn ceremony_instance_state_to_json(mut state: pb::CeremonyInstanceState) -> Value {
    let step_deadlines = std::mem::take(&mut state.step_deadlines)
        .into_iter()
        .map(|deadline| {
            json!({
                "step_id": deadline.step_id,
                "state_visit": deadline.state_visit,
                "state_iteration": deadline.state_iteration,
                "step_iteration": deadline.step_iteration,
                "attempt": deadline.attempt,
                "claim_fence": deadline.claim_fence,
                "deadline_at": deadline.deadline_at,
                "finished_by_role_id": deadline.finished_by_role_id,
            })
        })
        .collect::<Vec<_>>();
    let reasons = std::mem::take(&mut state.reasons)
        .into_iter()
        .map(|reason| {
            json!({
                "from": record_ref_to_json(reason.from),
                "to": record_ref_to_json(reason.to),
                "kind": reason.kind,
                "why": reason.why,
                "confidence": reason.confidence,
                "asserted_by_role_id": empty_as_null(reason.asserted_by_role_id),
                "asserted_at": reason.asserted_at,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "ceremony_id": state.ceremony_id,
        "budget_account_id": empty_as_null(state.budget_account_id),
        "trace_id": empty_as_null(state.trace_id),
        "correlation_id": empty_as_null(state.correlation_id),
        "causation_id": empty_as_null(state.causation_id),
        "definition_name": state.definition_name,
        "definition_version": state.definition_version,
        "bound_definition_digest": empty_as_null(state.bound_definition_digest),
        "current_state": state.current_state,
        "current_state_iteration": state.current_state_iteration,
        "current_state_visit": state.current_state_visit,
        "state_repeat_max_iterations": if state.state_repeat_max_iterations == 0 { Value::Null } else { json!(state.state_repeat_max_iterations) },
        "state_repeat_condition_satisfied": state.state_repeat_condition_satisfied,
        "state_repeat_limit_reached": state.state_repeat_limit_reached,
        "completed": state.completed,
        "next_step_id": empty_as_null(state.next_step_id),
        "claimable_step_ids": state.claimable_step_ids,
        "waiting_for_human": state.waiting_for_human,
        "guard_deferrals": state
            .guard_deferrals
            .into_iter()
            .map(|deferral| guard_deferral_to_json(&deferral))
            .collect::<Vec<_>>(),
        "transitions": state
            .transitions
            .into_iter()
            .map(available_transition_to_json)
            .collect::<Vec<_>>(),
        "steps": state.steps.into_iter().map(step_state_to_json).collect::<Vec<_>>(),
        "interventions": state
            .interventions
            .into_iter()
            .map(intervention_to_json)
            .collect::<Vec<_>>(),
        "open_intervention_ids": state.open_intervention_ids,
        "context": optional_pb_struct_to_json(state.context),
        "participant_bindings": state
            .participant_bindings
            .into_iter()
            .map(|binding| json!({
                "role_id": binding.role_id,
                "specialty": binding.specialty,
                "bound_at": binding.bound_at,
            }))
            .collect::<Vec<_>>(),
        // What this session was told when it opened, or `null` when it
        // was told nothing. Proto says absence with message presence
        // and JSON says it with null; keeping the two apart here is
        // what stops "recalled nothing" reading as "recalled an empty
        // scope".
        "recollection": state.recollection.map(recollection_to_json),
        "lineage": state.lineage.as_ref().map(lineage_to_json),
        "child_groups": state
            .child_groups
            .into_iter()
            .map(child_group_to_json)
            .collect::<Vec<_>>(),
        "lifecycle": state.lifecycle,
        "end_reason": empty_as_null(state.end_reason),
        "paused_at": empty_as_null(state.paused_at),
        "ended_at": empty_as_null(state.ended_at),
        "ceremony_deadline_at": empty_as_null(state.ceremony_deadline_at),
        "state_deadline_at": empty_as_null(state.state_deadline_at),
        "step_deadlines": step_deadlines,
        // Both backends answer the same shape or neither does: the
        // parity gate is what says so, and it is the reason this had to
        // be added here the moment the embedded side grew it.
        "reasons": reasons,
    })
}

pub(crate) fn child_completion_to_json(completion: &pb::ChildCompletionState) -> Value {
    json!({
        "group_id": completion.group_id,
        "child_id": completion.child_id,
        "terminal_event_id": completion.terminal_event_id,
        "terminal_record_hash": bytes_to_hex(&completion.terminal_record_hash),
    })
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(DIGITS[usize::from(byte >> 4)] as char);
        encoded.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    encoded
}

fn lineage_to_json(lineage: &pb::CeremonyLineageState) -> Value {
    json!({
        "root_id": lineage.root_id,
        "parent_id": lineage.parent_id,
        "group_id": lineage.group_id,
        "position": lineage.position,
        "depth": lineage.depth,
        "remaining_depth": lineage.remaining_depth,
    })
}

fn child_group_to_json(group: pb::CeremonyChildGroupState) -> Value {
    json!({
        "group_id": group.group_id,
        "step_id": group.step_id,
        "state_visit": group.state_visit,
        "state_iteration": group.state_iteration,
        "step_iteration": group.step_iteration,
        "active_claim_fence": group.active_claim_fence,
        "adopted_claim_fence": group.adopted_claim_fence,
        "max_children": group.max_children,
        "max_depth": group.max_depth,
        "children": group.children.into_iter().map(|child| json!({
            "child_id": child.child_id,
            "position": child.position,
            "ceremony": child.ceremony,
            "version": child.version,
            "definition_digest": child.definition_digest,
            "context": optional_pb_struct_to_json(child.context),
            "lineage": child.lineage.as_ref().map(lineage_to_json),
            "recollection": child.recollection.map(recollection_to_json),
            "opened_at": child.opened_at,
        })).collect::<Vec<_>>(),
        "completions": group.completions.iter().map(child_completion_to_json).collect::<Vec<_>>(),
    })
}

fn recollection_to_json(recollection: pb::CeremonyRecollectionState) -> Value {
    json!({
        "scope": recollection.scope,
        "truncated": recollection.truncated,
        "entries": recollection
            .entries
            .into_iter()
            .map(|entry| json!({
                "entry_id": entry.entry_id,
                "kind": entry.kind,
                "summary": entry.summary,
                "from_ceremony_id": entry.from_ceremony_id,
                "observed_at": entry.observed_at,
            }))
            .collect::<Vec<_>>(),
    })
}

/// What a reason points at, with only the fields its kind uses.
///
/// Proto fills every field of a flat message; carrying the unset ones
/// out to a caller would offer a step id on an agenda item.
fn record_ref_to_json(record: Option<pb::CeremonyRecordRefState>) -> Value {
    let Some(record) = record else {
        return Value::Null;
    };
    match record.kind.as_str() {
        "step" => json!({ "kind": "step", "step_id": record.step_id }),
        "agenda_item" => json!({ "kind": "agenda_item", "agenda_item": record.agenda_item }),
        "contribution" => json!({
            "kind": "contribution",
            "agenda_item": record.agenda_item,
            "ordinal": record.ordinal,
        }),
        "guard_decision" => json!({ "kind": "guard_decision", "guard_name": record.guard_name }),
        "transition" => json!({ "kind": "transition", "ordinal": record.ordinal }),
        other => json!({ "kind": other }),
    }
}

fn step_state_to_json(step: pb::CeremonyStepState) -> Value {
    json!({
        "step_id": step.step_id,
        "state_id": step.state_id,
        "status": step.status,
        "attempt": step.attempt,
        "output": optional_pb_struct_to_json(step.output),
        "error": empty_as_null(step.error),
        "iteration": step.iteration,
        "state_iteration": step.state_iteration,
        "state_visit": step.state_visit,
        "execution_profile": optional_pb_struct_to_json(step.execution_profile),
        "repeat_condition_satisfied": step.repeat_condition_satisfied,
        "repeat_limit_reached": step.repeat_limit_reached,
        "repeat_max_iterations": if step.repeat_max_iterations == 0 {
            Value::Null
        } else {
            json!(step.repeat_max_iterations)
        },
    })
}

fn available_transition_to_json(transition: pb::CeremonyAvailableTransition) -> Value {
    json!({
        "trigger": transition.trigger,
        "to_state": transition.to,
        "enabled": transition.enabled,
        "guards": transition
            .guards
            .into_iter()
            .map(|guard| json!({
                "name": guard.name,
                "kind": guard.kind,
                "satisfied": guard.satisfied,
            }))
            .collect::<Vec<_>>(),
    })
}

fn guard_deferral_to_json(deferral: &pb::CeremonyGuardDeferralState) -> Value {
    json!({
        "guard_name": deferral.guard_name,
        "statement": deferral.statement,
        "reason": deferral.reason,
        "reconsider_when": deferral.reconsider_when.clone(),
        "deferred_at": deferral.deferred_at,
    })
}

fn intervention_to_json(intervention: pb::CeremonyInterventionState) -> Value {
    json!({
        "intervention_id": intervention.intervention_id,
        "kind": intervention.kind,
        "status": intervention.status,
        "requested_by": intervention.requested_by,
        "target": intervention.target.as_ref().map_or_else(
            || json!({ "kind": "table" }),
            intervention_target_to_json,
        ),
        "request": intervention.request.map_or_else(
            || json!({ "message": "", "details": {} }),
            intervention_message_to_json,
        ),
        "provenance": intervention.provenance.map(|provenance| json!({
            "source_intervention_id": provenance.source_intervention_id,
            "source_response_role_id": provenance.source_response_role_id,
            "selected_role_id": provenance.selected_role_id,
        })),
        "responses": intervention
            .responses
            .into_iter()
            .map(|response| json!({
                "role_id": response.role_id,
                "message": response.content.as_ref().map_or("", |c| c.message.as_str()),
                "details": optional_pb_struct_to_json(
                    response.content.and_then(|content| content.details),
                ),
                "evidence_pack": evidence_pack_to_json(response.evidence_pack),
                "responded_at": response.responded_at,
            }))
            .collect::<Vec<_>>(),
        "created_at": intervention.created_at,
        "updated_at": intervention.updated_at,
        "closed_at": empty_as_null(intervention.closed_at),
    })
}

/// An item put to the whole table carries no roles, and says so by
/// having no `role_ids` at all rather than an empty list. Proto has no
/// way to leave a repeated field out, so the distinction is restored
/// here — an empty list reads as "put to nobody", which is the one
/// thing a target can never mean.
fn intervention_target_to_json(target: &pb::CeremonyInterventionTargetState) -> Value {
    if target.role_ids.is_empty() {
        json!({ "kind": target.kind })
    } else {
        json!({
            "kind": target.kind,
            "role_ids": target.role_ids.clone(),
        })
    }
}

fn intervention_message_to_json(message: pb::CeremonyInterventionMessage) -> Value {
    json!({
        "message": message.message,
        "details": optional_pb_struct_to_json(message.details),
    })
}

/// Proto cannot say "absent", so an empty string is how absence
/// arrives. Turning it back into `null` is what makes the two backends
/// answer the same thing.
/// The pack a source returned, as the object it is.
///
/// Proto carries it as the serialized document, because a pack is a
/// versioned record rather than a bag of fields; the in-process arm
/// never serializes it and answers the object. One tool answering a
/// string on one backend and an object on the other is a client that
/// works until it is pointed at the other engine, so the string is
/// read back here. A payload that will not parse is handed on
/// untouched rather than silently dropped.
fn evidence_pack_to_json(value: String) -> Value {
    if value.is_empty() {
        return Value::Null;
    }
    serde_json::from_str(&value).unwrap_or(Value::String(value))
}

fn empty_as_null(value: String) -> Value {
    if value.is_empty() {
        Value::Null
    } else {
        Value::String(value)
    }
}
