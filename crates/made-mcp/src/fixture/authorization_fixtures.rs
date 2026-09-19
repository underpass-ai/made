use serde_json::{json, Value};

pub(super) fn approval() -> Value {
    json!({
        "decision": {
            "decision_id":"fixture-approval", "request_id":"fixture-request",
            "principal":{"principal_id":"fixture-approver","kind":"human","authentication_method":"mutual_tls"},
            "action":"approve_ceremony_guard", "approved_action":"complete_ceremony_step",
            "scope":{"kind":"global"}, "target_digest":"a".repeat(64),
            "approval_decision_id":null, "accepted_work_decision_id":null,
            "policy_version":4, "outcome":"allow", "grant_id":"fixture-grant",
            "denial_reason":null, "decided_at":"1970-01-01T00:00:00Z",
            "valid_until":"1970-01-01T00:01:00Z"
        }
    })
}

pub(super) fn decisions() -> Value {
    json!({"decisions":[],"next_after_decision_id":null})
}
