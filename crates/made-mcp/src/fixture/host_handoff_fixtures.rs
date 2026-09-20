use serde_json::{json, Value};

pub(super) fn response(name: &str) -> Value {
    match name {
        "made_record_ceremony_host_handoff" => {
            json!({"declaration":null,"recorded_at":"2026-09-19T12:00:00Z"})
        }
        "made_inspect_ceremony_resume" => {
            json!({"admission_paused":true,"engine_drained":false,"all_claims_host_reported_quiesced":false,"claims":[]})
        }
        _ => unreachable!("only host handoff tools reach this fixture"),
    }
}
