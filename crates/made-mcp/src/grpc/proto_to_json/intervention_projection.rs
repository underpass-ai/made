//! One intervention, rendered from the versioned contract.
//!
//! Split from the instance projection because an item now carries its
//! routing, its intent and what hosts said about it, and because all
//! three renderers here follow one rule: a shape names what it names,
//! and the keys it does not use are absent rather than null.

use made_mcp_proto::v1 as pb;
use serde_json::{json, Value};

use super::optional_pb_struct_to_json;

/// An item put to the whole table carries no roles, and says so by
/// having no `role_ids` at all rather than an empty list. Proto has no
/// way to leave a repeated field out, so the distinction is restored
/// here — an empty list reads as "put to nobody", which is the one
/// thing a target can never mean.
pub(super) fn intervention_target_to_json(target: &pb::CeremonyInterventionTargetState) -> Value {
    let mut value = json!({ "kind": target.kind });
    let object = value
        .as_object_mut()
        .expect("a target renders as an object");
    if !target.role_ids.is_empty() {
        object.insert("role_ids".to_owned(), json!(target.role_ids.clone()));
    }
    // The same rule for the exact shape: the three fields that name one
    // live agent are present together or not at all.
    if !target.agent_execution_id.is_empty() {
        object.insert(
            "agent_execution_id".to_owned(),
            json!(target.agent_execution_id.clone()),
        );
        object.insert("incarnation".to_owned(), json!(target.incarnation.clone()));
        object.insert("role_id".to_owned(), json!(target.role_id.clone()));
    }
    value
}

/// One answer, with who gave it only when an agent was handed the item.
///
/// The three provenance fields are present together or not at all, the
/// same rule a target follows: a seat that answered without being
/// handed anything has no executor, and three nulls saying so would be
/// a shape a caller has to learn to ignore.
pub(super) fn intervention_response_to_json(
    response: pb::CeremonyInterventionResponseState,
) -> Value {
    let mut value = json!({
        "role_id": response.role_id,
        "message": response.content.as_ref().map_or("", |c| c.message.as_str()),
        "details": optional_pb_struct_to_json(
            response.content.and_then(|content| content.details),
        ),
        "evidence_pack": evidence_pack_to_json(response.evidence_pack),
        "responded_at": response.responded_at,
    });
    if !response.executor_agent_execution_id.is_empty() {
        let object = value.as_object_mut().expect("a response is an object");
        object.insert(
            "executor_agent_execution_id".to_owned(),
            json!(response.executor_agent_execution_id),
        );
        object.insert(
            "executor_incarnation".to_owned(),
            json!(response.executor_incarnation),
        );
        object.insert("delivery_id".to_owned(), json!(response.delivery_id));
    }
    value
}

pub(super) fn intervention_message_to_json(message: pb::CeremonyInterventionMessage) -> Value {
    json!({
        "message": message.message,
        "details": optional_pb_struct_to_json(message.details),
    })
}

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
