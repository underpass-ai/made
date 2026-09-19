use serde_json::{json, Value};

pub(super) fn budget_limits_schema() -> Value {
    json!({
        "type":"object", "additionalProperties":false,
        "properties":{
            "duration_micros":{"type":"integer","minimum":1},
            "tokens":{"type":"integer","minimum":1},
            "cost_micros":{"type":"integer","minimum":1},
            "tool_calls":{"type":"integer","minimum":1},
            "currency":{"type":"string","pattern":"^[A-Z]{3}$"}
        },
        "anyOf":[
            {"required":["duration_micros"]},{"required":["tokens"]},
            {"required":["cost_micros"]},{"required":["tool_calls"]}
        ]
    })
}

pub(super) fn budget_reservation_schema() -> Value {
    let measurement = json!({
        "type":"object", "additionalProperties":false,
        "required":["quality"],
        "properties":{
            "quality":{"type":"string","enum":["observed","estimated","unknown"]},
            "amount":{"type":"integer","minimum":0}
        },
        "if":{"properties":{"quality":{"enum":["observed","estimated"]}}},
        "then":{"required":["amount"]},
        "else":{"properties":{"amount":{"const":0}}}
    });
    json!({
        "type":"object", "additionalProperties":false,
        "required":["duration","tokens","cost","tool_calls"],
        "properties":{
            "duration":measurement.clone(), "tokens":measurement.clone(),
            "cost":measurement.clone(), "tool_calls":measurement
        }
    })
}

pub(super) fn budget_report_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"required":["ceremony_id"],
        "properties":{"ceremony_id":{"type":"string","minLength":1}}})
}

pub(super) fn pending_budget_reservations_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"properties":{
        "after_reservation_id":{"type":"string","minLength":1},
        "limit":{"type":"integer","minimum":1,"maximum":500}
    }})
}
