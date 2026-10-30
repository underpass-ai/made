use super::tool_def;
use serde_json::{json, Value};

fn lease_schema() -> Value {
    json!({"type":"object", "additionalProperties":false,
    "required":["consumer","id","expires_at"], "properties":{
        "consumer":{"type":"string","minLength":1,"maxLength":128},
        "id":{"type":"string","minLength":1,"maxLength":128},
        "acknowledged_through":{"type":["integer","null"],"minimum":1},
        "expires_at":{"type":"string","format":"date-time"}
    }})
}
pub(super) fn council_journal_tool_catalog() -> Vec<Value> {
    vec![
        tool_def("made_read_council_events", "Read a bounded page of the independent council journal without advancing a consumer cursor.", json!({
            "type":"object","additionalProperties":false,"properties":{
                "after":{"type":"integer","minimum":1},
                "limit":{"type":"integer","minimum":1,"maximum":1000}
            }})),
        tool_def("made_get_council_event_cursor", "Read the durable acknowledgement position for one council journal consumer.", json!({
            "type":"object","additionalProperties":false,"required":["consumer"],
            "properties":{"consumer":{"type":"string","minLength":1,"maxLength":128}}})),
        tool_def("made_lease_council_events", "Acquire an exclusive council consumer lease. A busy consumer returns a null lease; no progress is acknowledged.", json!({
            "type":"object","additionalProperties":false,"required":["consumer","duration_ms"],
            "properties":{"consumer":{"type":"string","minLength":1,"maxLength":128},
                "duration_ms":{"type":"integer","minimum":1,"maximum":3_600_000}}})),
        tool_def("made_acknowledge_council_events", "Acknowledge a delivered council journal position using the complete current lease. The store rejects expired or replaced leases.", json!({
            "type":"object","additionalProperties":false,"required":["lease","through"],
            "properties":{"lease":lease_schema(),"through":{"type":"integer","minimum":1}}})),
        tool_def("made_release_council_events", "Release a council consumer lease without advancing its acknowledgement position.", json!({
            "type":"object","additionalProperties":false,"required":["lease"],
            "properties":{"lease":lease_schema()}})),
    ]
}
