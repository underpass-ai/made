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

fn delivery(state: pb::CeremonyInterventionDeliveryState) -> Value {
    json!({
        "intervention": state.intervention.map(intervention),
        "routes": state.routes.into_iter().map(route).collect::<Vec<_>>(),
        "status_delivery": state.status,
        "status_reason": optional(state.status_reason),
        "unresolved": state.unresolved,
    })
}

fn intervention(state: pb::CeremonyInterventionState) -> Value {
    json!({
        "intervention_id": state.intervention_id,
        "kind": state.kind,
        "status": state.status,
        "requested_by": state.requested_by,
        "intent": optional(state.intent),
        "target": state.target.map(|target| json!({
            "kind": target.kind,
            "role_ids": target.role_ids,
            "agent_execution_id": optional(target.agent_execution_id),
            "incarnation": optional(target.incarnation),
            "role_id": optional(target.role_id),
        })),
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
