use serde_json::{json, Value};

pub(super) fn response(name: &str) -> Value {
    match name {
        "made_plan_protocol_successor" => {
            json!({"ready":false,"blockers":["a protocol hands off from a pause; this one is not paused"],"proposed_carried":[],"required_dispositions":[],"strands":[]})
        }
        "made_start_protocol_successor" => {
            json!({"successor_id":"parity-session.s.0000000000000000","plan":null})
        }
        _ => unreachable!("only succession tools reach this fixture"),
    }
}
