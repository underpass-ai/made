use serde_json::{json, Value};

pub(super) fn receipt() -> Value {
    json!({
        "receipt_id": "1".repeat(64),
        "operation_id": "2".repeat(64),
        "request_digest": "3".repeat(64),
        "producer_claim_fence": "4".repeat(64),
        "connector_id": "fixture",
        "external_operation_id": null,
        "recovery_capability": "idempotent_by_operation_id",
        "source_kind": "fixture",
        "status": "completed",
        "output": {"fixture": true},
        "error": null,
        "artifacts": [],
        "observed_at": "1970-01-01T00:00:00Z"
    })
}

pub(super) fn recovery_page() -> Value {
    json!({
        "items": [{
            "operation_id": "2".repeat(64),
            "ceremony_id": "fixture-ceremony",
            "step_id": "fixture-step",
            "request_digest": "3".repeat(64),
            "intents": [{
                "claim_fence": "4".repeat(64),
                "connector_id": "fixture",
                "recovery_capability": "idempotent_by_operation_id",
                "source_kind": "fixture",
                "actor_kind": "agent",
                "recorded_at": "1970-01-01T00:00:00Z"
            }],
            "receipt": receipt(),
            "current_claim_fence": "4".repeat(64)
        }],
        "next_cursor": null
    })
}
