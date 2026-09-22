use serde_json::{json, Value};

/// What the loop looks like from the fixture backend.
///
/// One binding, one item leased to it and not yet answered: the state
/// a reader is most likely to misread. `state` says `leased`, which
/// means a host was handed it, and the absent `action_kind` is what
/// says nobody has yet said what they would do about it.
pub(super) fn handles(name: &str) -> bool {
    matches!(
        name,
        "made_bind_ceremony_integrator"
            | "made_get_ceremony_integrator_binding"
            | "made_await_integrator_attention"
            | "made_acknowledge_integrator_attention"
            | "made_list_attention_deliveries"
    )
}

pub(super) fn response(name: &str) -> Value {
    match name {
        "made_bind_ceremony_integrator" => json!({
            "outcome": "bound",
            "binding": binding(),
            "previous": null,
        }),
        "made_get_ceremony_integrator_binding" => json!({ "binding": binding() }),
        "made_await_integrator_attention" => json!({
            "items": [delivery()],
            "loop_state": "awaiting_results",
            "journal_head": null,
            "end_reason": "items",
        }),
        "made_acknowledge_integrator_attention" => json!({
            "acknowledgement": "intent",
            "outcome": "acknowledged",
            "action_kind": "integrated",
            "idempotency_key": "fixture-integration-1",
            "delivery": record("acknowledged"),
            "conflict": null,
        }),
        "made_list_attention_deliveries" => json!({
            "deliveries": [record("leased")],
            "next_cursor": null,
        }),
        _ => unreachable!("only integrator loop calls reach this fixture"),
    }
}

fn binding() -> Value {
    json!({
        "binding_id": "binding-1",
        "scope": { "kind": "ceremony", "ceremony_id": "ceremony-1" },
        "role_id": "INTEGRATOR",
        "host_kind": "generic",
        "address": "session-1",
        "activation": "none",
        "incarnation": "run-1",
        "fence": 0,
        "bound_at": "2026-09-20T12:00:00Z",
        "revoked_at": null,
    })
}

fn delivery() -> Value {
    json!({
        "delivery_id": "ceremony-1:attention:attention-1:binding:binding-1",
        "lease_id": "22222222-2222-4222-8222-222222222222",
        "leased_until": "2026-09-20T12:01:00Z",
        "attention": {
            "attention_id": "attention-1",
            "kind": "result_available",
            "ceremony_id": "ceremony-1",
            "system_execution_id": null,
            "step_id": "investigate",
            "position": 7,
            "occurred_at": "2026-09-20T12:00:00Z",
            "reason": "a step produced a result this integrator can take up",
            "evidence": [],
            "acceptance": "accepted",
            "source_event_id": "event-7",
            "source_sequence": 7,
        },
        "context": {
            "ceremony_id": "ceremony-1",
            "lifecycle": "running",
            "current_state": "INVESTIGATING",
            "claimable_step_ids": [],
            "waiting_for_human": [],
        },
    })
}

fn record(state: &str) -> Value {
    json!({
        "delivery_id": "ceremony-1:attention:attention-1:binding:binding-1",
        "binding_id": "binding-1",
        "ceremony_id": "ceremony-1",
        "attention_id": "attention-1",
        "state": state,
        "attempt": 1,
        "lease_id": "22222222-2222-4222-8222-222222222222",
        "leased_until": "2026-09-20T12:01:00Z",
        "action_kind": null,
        "idempotency_key": null,
        "failure_reason": null,
        "created_at": "2026-09-20T12:00:00Z",
        "updated_at": "2026-09-20T12:00:30Z",
    })
}
