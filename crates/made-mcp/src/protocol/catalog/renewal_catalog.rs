use crate::protocol::schema_primitives::{string_schema, tool_def};
use serde_json::{json, Value};

pub(super) fn renewal_tool() -> Value {
    tool_def(
        crate::protocol::tool_names::RENEW_CEREMONY_STEP_LEASE_TOOL,
        "Renew a live delegated claim using its original owner and claim_fence. Reuse renewal_id only to retry the identical request: replay returns the original receipt and does not extend again. Absolute deadlines and current authorization still apply; renewal does not prove host-agent liveness.",
        json!({"type":"object", "additionalProperties":false,
            "required":["ceremony_id","step_id","claim_fence","lease_owner_id","renewal_id","lease_ttl_ms"],
            "properties":{
                "ceremony_id":string_schema("Ceremony containing the accepted claim."),
                "step_id":string_schema("Claimed step."),
                "claim_fence":string_schema("Original accepted producer fence; never a replacement fence."),
                "lease_owner_id":string_schema("Original logical lease owner."),
                "renewal_id":string_schema("Stable unique identity for this heartbeat; reuse for response-loss retries."),
                "lease_ttl_ms":{"type":"integer","minimum":1,"maximum":9_007_199_254_740_991_u64,"description":"Requested lifetime from server acceptance; capped by absolute deadlines."}
            }}),
    )
}
