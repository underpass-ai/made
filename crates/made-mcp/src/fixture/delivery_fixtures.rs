use serde_json::{json, Value};

/// What a host sees from the fixture backend.
///
/// Deliberately shows one route that has been leased and not yet
/// acknowledged, because that is the state a reader is most likely to
/// misread: `status_delivery` says `delivered`, which means a host was
/// handed it, and the empty `deliveries` list is what says nobody has
/// yet reported seeing it.
pub(super) fn handles(name: &str) -> bool {
    matches!(
        name,
        "made_pull_ceremony_agent_interventions"
            | "made_acknowledge_ceremony_agent_intervention"
            | "made_get_ceremony_intervention"
            | "made_list_ceremony_interventions"
    )
}

pub(super) fn response(name: &str) -> Value {
    match name {
        // Answering with the whole session, as every other move does.
        "made_acknowledge_ceremony_agent_intervention" => json!({
            "ceremony_id": "ceremony-1",
            "interventions": [intervention("acknowledged")],
        }),
        "made_pull_ceremony_agent_interventions" => json!({
            "items": [{
                "delivery_id": "ceremony-1:intervention:item-1:agent:exec-1:inc-1",
                "lease_id": "11111111-1111-4111-8111-111111111111",
                "leased_until": "2026-09-20T12:01:00Z",
                "intervention": intervention("delivered"),
            }]
        }),
        "made_get_ceremony_intervention" => intervention("delivered"),
        "made_list_ceremony_interventions" => json!({
            "interventions": [intervention("delivered")],
            "next_cursor": null,
        }),
        _ => unreachable!("only intervention delivery reads reach this fixture"),
    }
}

fn intervention(status: &str) -> Value {
    json!({
        "intervention": {
            "intervention_id": "item-1",
            "kind": "opinion",
            "status": "open",
            "requested_by": "LEAD",
            "intent": "question",
            "target": {
                "kind": "agent_execution",
                "role_ids": null,
                "agent_execution_id": "exec-1",
                "incarnation": "inc-1",
                "role_id": "ENGINEER"
            },
            "message": "Is the migration still reversible?",
            "supervisor": null,
            "responses": [],
            "deliveries": [],
            "created_at": "2026-09-20T12:00:00Z",
            "updated_at": "2026-09-20T12:00:00Z",
            "closed_at": null
        },
        "routes": [{
            "delivery_id": "ceremony-1:intervention:item-1:agent:exec-1:inc-1",
            "target": {
                "kind": "agent_execution",
                "agent_execution_id": "exec-1",
                "incarnation": "inc-1",
                "role_id": null
            },
            "state": "leased",
            "attempt": 0,
            "lease_id": "11111111-1111-4111-8111-111111111111",
            "leased_until": "2026-09-20T12:01:00Z",
            "observation_kind": null,
            "observation_note": null,
            "observed_at": null
        }],
        "status_delivery": status,
        "status_reason": null,
        "unresolved": true
    })
}
