use serde_json::{json, Value};

const FIXTURE_DIGEST: &str = "3f786850e387550fdab836ed7e6dc881de23001b";

pub(super) fn response(name: &str) -> Value {
    match name {
        "made_list_protocol_definitions" => json!({
            "definitions": [{
                "protocol": "fixture_protocol",
                "version": "1.0",
                "digest": FIXTURE_DIGEST,
                "description": null,
                "state_count": 2,
                "step_count": 1
            }],
            "next_cursor": null
        }),
        "made_get_protocol_definition" => json!({
            "protocol": "fixture_protocol",
            "version": "1.0",
            "digest": FIXTURE_DIGEST,
            "definition_yaml": "version: '1.0'\nname: fixture_protocol\n"
        }),
        _ => unreachable!("only catalogue reads reach this fixture"),
    }
}
