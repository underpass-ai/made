use made_mcp_proto::v1 as pb;
use serde_json::{json, Value};

fn optional(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

pub(super) fn leases(items: Vec<pb::CeremonyAgentInterventionLeaseState>) -> Value {
    json!({ "items": items.into_iter().map(lease).collect::<Vec<_>>() })
}

fn lease(state: pb::CeremonyAgentInterventionLeaseState) -> Value {
    json!({
        "delivery_id": state.delivery_id,
        "lease_id": state.lease_id,
        "leased_until": state.leased_until,
        "intervention": state.delivery.map(delivery),
    })
}

pub(super) fn one(state: Option<pb::CeremonyInterventionDeliveryState>) -> Value {
    state.map_or(Value::Null, delivery)
}

pub(super) fn page(
    interventions: Vec<pb::CeremonyInterventionDeliveryState>,
    next_cursor: String,
) -> Value {
    json!({
        "interventions": interventions.into_iter().map(delivery).collect::<Vec<_>>(),
        "next_cursor": optional(next_cursor),
    })
}

/// One flat object, not an item nested inside a wrapper.
///
/// The embedded backend answers the item's own fields at the top level
/// with the routes beside them, and the parity gate holds the two
/// backends to one shape: a caller should not have to know which engine
/// served it to know where to look.
fn delivery(state: pb::CeremonyInterventionDeliveryState) -> Value {
    let mut value = state.intervention.map_or_else(|| json!({}), intervention);
    let object = value
        .as_object_mut()
        .expect("an intervention renders as an object");
    object.insert(
        "routes".to_owned(),
        Value::Array(state.routes.into_iter().map(route).collect()),
    );
    object.insert("status_delivery".to_owned(), Value::String(state.status));
    object.insert(
        "status_reason".to_owned(),
        optional(state.status_reason).map_or(Value::Null, Value::String),
    );
    object.insert("unresolved".to_owned(), Value::Bool(state.unresolved));
    value
}

fn intervention(state: pb::CeremonyInterventionState) -> Value {
    json!({
        "intervention_id": state.intervention_id,
        "kind": state.kind,
        "status": state.status,
        "requested_by": state.requested_by,
        "intent": optional(state.intent),
        "target": state.target.as_ref().map(target_state),
        "provenance": Value::Null,
        "message": state.request.map(|request| request.message),
        "supervisor": optional(state.supervisor_principal_id).map(|principal_id| json!({
            "principal_id": principal_id,
            "display": state.supervisor_display,
        })),
        "responses": state.responses.into_iter().map(|response| json!({
            "role_id": response.role_id,
            "message": response.content.map(|content| content.message),
            "responded_at": response.responded_at,
            "executor_agent_execution_id": optional(response.executor_agent_execution_id),
            "executor_incarnation": optional(response.executor_incarnation),
            "delivery_id": optional(response.delivery_id),
        })).collect::<Vec<_>>(),
        "deliveries": state.deliveries.into_iter().map(|ack| json!({
            "delivery_id": ack.delivery_id,
            "agent_execution_id": ack.agent_execution_id,
            "incarnation": ack.incarnation,
            "role_id": ack.role_id,
            "observation_kind": ack.observation_kind,
            "observation_note": ack.observation_note,
            "acknowledged_at": ack.acknowledged_at,
        })).collect::<Vec<_>>(),
        "created_at": state.created_at,
        "updated_at": state.updated_at,
        "closed_at": optional(state.closed_at),
    })
}

/// A target names what it names; unused keys are absent, not null.
fn target_state(target: &pb::CeremonyInterventionTargetState) -> Value {
    let mut value = json!({ "kind": target.kind });
    let object = value
        .as_object_mut()
        .expect("a target renders as an object");
    if !target.role_ids.is_empty() {
        object.insert("role_ids".to_owned(), json!(target.role_ids.clone()));
    }
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

fn route(state: pb::CeremonyInterventionDeliveryRouteState) -> Value {
    json!({
        "delivery_id": state.delivery_id,
        "target": {
            "kind": state.target_kind,
            "agent_execution_id": optional(state.target_agent_execution_id),
            "incarnation": optional(state.target_incarnation),
            "role_id": optional(state.target_role_id),
        },
        "state": state.state,
        "attempt": state.attempt,
        "lease_id": optional(state.lease_id),
        "leased_until": optional(state.leased_until),
        "observation_kind": optional(state.observation_kind),
        "observation_note": optional(state.observation_note),
        "observed_at": optional(state.observed_at),
    })
}
