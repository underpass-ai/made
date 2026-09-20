use super::BUDGET_SESSION_ID;
use serde_json::{json, Value};

pub(super) fn script() -> Vec<(&'static str, Value)> {
    let declaration = json!({"id":"preflight-ack","step_id":"work","owner":"parity-budget-host",
        "incarnation":"parity-task-1","state":"quiesced","observed_at":"2026-09-16T09:00:00Z",
        "evidence":"artifact:parity-handoff"});
    vec![
        (
            "made_pause_ceremony",
            json!({"ceremony_id":BUDGET_SESSION_ID,"actor_id":"operator","actor_kind":"service","reason":"handoff"}),
        ),
        (
            "made_inspect_ceremony_resume",
            json!({"ceremony_id":BUDGET_SESSION_ID}),
        ),
        (
            "made_record_ceremony_host_handoff",
            json!({"ceremony_id":BUDGET_SESSION_ID,"declaration":declaration}),
        ),
        (
            "made_record_ceremony_host_handoff",
            json!({"ceremony_id":BUDGET_SESSION_ID,"declaration":declaration}),
        ),
        (
            "made_inspect_ceremony_resume",
            json!({"ceremony_id":BUDGET_SESSION_ID}),
        ),
    ]
}
