use serde_json::{json, Value};

pub(super) fn response(name: &str) -> Value {
    match name {
        "made_plan_ceremony_successor" => {
            json!({"ready":false,"blockers":["a ceremony hands off from a pause; this one is not paused"],"proposed_carried":[],"required_dispositions":[],"strands":[]})
        }
        "made_start_ceremony_successor" => {
            json!({"successor":null,"plan":null})
        }
        _ => unreachable!("only succession tools reach this fixture"),
    }
}
