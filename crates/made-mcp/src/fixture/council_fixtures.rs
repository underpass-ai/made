use serde_json::{json, Value};

pub(super) fn deliberate_fixture() -> Value {
    json!({
        "task_id": "task-fixture-1",
        "winner_proposal_id": "proposal-fixture-a",
        "duration_ms": 42,
        "results": [
            {
                "rank": 0,
                "proposal": {
                    "proposal_id": "proposal-fixture-a",
                    "author_agent_id": "agent-fixture-1",
                    "content": "fixture answer",
                    "metadata": {},
                    "revision_count": 0
                },
                "validation": {
                    "score": 1.0,
                    "reports": [
                        { "kind": "content-non-empty", "passed": true, "summary": "ok", "details": {} }
                    ]
                }
            }
        ],
        "metadata": { "fixture": true }
    })
}

pub(super) fn stream_fixture() -> Value {
    json!({
        "task_id": "task-fixture-1",
        "frames": [
            { "phase": "DELIBERATION_PHASE_PROPOSING", "emitted_at": null, "payload": null },
            { "phase": "DELIBERATION_PHASE_REVISING", "emitted_at": null, "payload": null },
            { "phase": "DELIBERATION_PHASE_VALIDATING", "emitted_at": null, "payload": null },
            { "phase": "DELIBERATION_PHASE_SCORING", "emitted_at": null, "payload": null },
            {
                "phase": "DELIBERATION_PHASE_COMPLETED",
                "emitted_at": null,
                "payload": { "kind": "result", "result": deliberate_fixture()["results"][0].clone() }
            }
        ],
        "winner": deliberate_fixture()["results"][0].clone()
    })
}

pub(super) fn get_deliberation_fixture() -> Value {
    json!({
        "found": true,
        "result": deliberate_fixture()
    })
}

pub(super) fn orchestrate_fixture() -> Value {
    json!({
        "task_id": "task-fixture-1",
        "execution_id": "exec-fixture-1",
        "duration_ms": 73,
        "winner": deliberate_fixture()["results"][0].clone(),
        "candidates": [],
        "metadata": { "fixture": true }
    })
}
