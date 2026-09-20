use serde_json::{json, Value};

pub(super) fn one() -> Value {
    json!({
        "ceremony_id": "ceremony-fixture-1", "agent_execution_id": "execution-fixture-1",
        "operation_id": "1111111111111111111111111111111111111111111111111111111111111111",
        "claim_owner_id": "host-fixture-1",
        "logical_worker_id": "participant-fixture-1", "host_agent_id": "host-fixture-1",
        "host_agent_incarnation": "incarnation-fixture-1",
        "previous_host_agent_id": null, "previous_host_agent_incarnation": null,
        "role_id": "reviewer", "step_id": "review", "attempt": 1,
        "execution_status": "running", "liveness": "fresh", "source": "host_report",
        "requested_model": null, "requested_reasoning_effort": null,
        "actual_model": null, "actual_reasoning_effort": null,
        "activity": "reviewing", "blocker": null, "dependency": null,
        "task_summary": "Review the bounded fixture task.", "evidence_references": [],
        "usage_kind": "unavailable", "usage_value": null,
        "observed_at": "1970-01-01T00:00:00Z", "report_sequence": 1,
        "idempotency_key": "fixture-agent-status-1", "claim_fence": "2222222222222222222222222222222222222222222222222222222222222222"
    })
}

pub(super) fn list() -> Value {
    json!({"agents": [one()], "next_cursor": null})
}
