use serde_json::{json, Value};

use super::ceremony_instance_fixture;

pub(super) fn prepare_ceremony_children_fixture() -> Value {
    json!({
        "instance": ceremony_instance_fixture(),
        "child_group_id": "child-group-fixture-1",
        "child_ids": ["child-fixture-1", "child-fixture-2"]
    })
}

pub(super) fn accept_child_completion_fixture() -> Value {
    json!({
        "parent": ceremony_instance_fixture(),
        "completion": {
            "group_id": "child-group-fixture-1",
            "child_id": "child-fixture-1",
            "terminal_event_id": "child-fixture-1:completed",
            "terminal_record_hash": "00".repeat(32)
        }
    })
}

pub(super) fn recover_ceremony_children_fixture() -> Value {
    json!({
        "recovered_plans": 1,
        "accepted_completions": 1,
        "skipped": 0,
        "failed": 0,
        "busy": false
    })
}
