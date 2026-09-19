use serde_json::{json, Value};

use super::{authorization_actions::GRANT_ACTIONS, schema_primitives::string_schema};

pub(super) fn grant_schema() -> Value {
    json!({
        "type":"object", "additionalProperties":false,
        "required":["grant_id","grantee_id","actions","scope","valid_from","delegation_depth"],
        "properties":{
            "grant_id":string_schema("Stable grant identity; retries must preserve its contents."),
            "grantee_id":string_schema("Principal receiving authority. Issuer identity comes from the authenticated channel."),
            "actions":{"type":"array","minItems":1,"maxItems":100,"uniqueItems":true,"items":{"type":"string","enum":GRANT_ACTIONS}},
            "scope":scope_schema(),
            "valid_from":string_schema("Inclusive RFC3339 validity start."),
            "valid_until":string_schema("Exclusive RFC3339 validity end; omit for no expiry."),
            "delegation_depth":{"type":"integer","minimum":0,"maximum":8},
            "parent_grant_id":string_schema("Parent grant for delegated authority; omit for a direct grant.")
        }
    })
}

fn scope_schema() -> Value {
    let id = string_schema("Exact resource identity.");
    json!({"oneOf":[
        {"type":"object","additionalProperties":false,"required":["kind"],"properties":{"kind":{"enum":["global"]}}},
        {"type":"object","additionalProperties":false,"required":["kind","ceremony_id"],"properties":{"kind":{"enum":["ceremony"]},"ceremony_id":id}},
        {"type":"object","additionalProperties":false,"required":["kind","root_id"],"properties":{"kind":{"enum":["ceremony_tree"]},"root_id":id}},
        {"type":"object","additionalProperties":false,"required":["kind","name"],"properties":{"kind":{"enum":["definition"]},"name":id,"version":{"oneOf":[{"type":"string","minLength":1},{"type":"null"}]}}},
        {"type":"object","additionalProperties":false,"required":["kind","artifact_id"],"properties":{"kind":{"enum":["artifact"]},"artifact_id":id}},
        {"type":"object","additionalProperties":false,"required":["kind","council_id"],"properties":{"kind":{"enum":["council"]},"council_id":id}},
        {"type":"object","additionalProperties":false,"required":["kind","account_id"],"properties":{"kind":{"enum":["budget"]},"account_id":id}}
    ]})
}

pub(super) fn approval_schema() -> Value {
    json!({
        "type":"object", "additionalProperties":false,
        "required":["approval_action","execution_action","scope","target_digest"],
        "properties":{
            "approval_action":{"type":"string","enum":GRANT_ACTIONS},
            "execution_action":{"type":"string","enum":GRANT_ACTIONS},
            "scope":scope_schema(),
            "target_digest":{"type":"string","pattern":"^[0-9a-f]{64}$"}
        }
    })
}

pub(super) fn revoke_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"required":["grant_id","reason"],"properties":{
        "grant_id":string_schema("Grant to revoke."),
        "reason":string_schema("Reason recorded in the policy journal.")
    }})
}

pub(super) fn decisions_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"properties":{
        "after_decision_id":string_schema("Exclusive decision cursor returned by the previous page."),
        "limit":{"type":"integer","minimum":1,"maximum":500}
    }})
}
