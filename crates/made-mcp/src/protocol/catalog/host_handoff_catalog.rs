use crate::protocol::schema_primitives::{string_schema, tool_def};
use crate::protocol::tool_names::{
    INSPECT_CEREMONY_RESUME_TOOL, RECORD_CEREMONY_HOST_HANDOFF_TOOL,
};
use serde_json::{json, Value};

pub(super) fn record_tool() -> Value {
    tool_def(RECORD_CEREMONY_HOST_HANDOFF_TOOL,
        "Journal an authenticated external host declaration for the exact current owner, claim fence and worker incarnation. Requires a timestamp and evidence reference; idempotent id with identical payload only. Does not stop a process, retire a claim, renew clocks or authorize takeover.",
        json!({"type":"object","additionalProperties":false,"required":["ceremony_id","declaration"],
          "properties":{"ceremony_id":string_schema("Ceremony holding the claim."),
            "declaration":{"type":"object","additionalProperties":false,
              "required":["id","step_id","claim_fence","owner","incarnation","state","observed_at","evidence"],
              "properties":{
                "id":{"type":"string","minLength":1,"maxLength":256},
                "step_id":string_schema("Claimed step."),
                "claim_fence":string_schema("Original accepted claim fence."),
                "owner":{"type":"string","minLength":1,"maxLength":256},
                "incarnation":{"type":"string","minLength":1,"maxLength":256},
                "state":{"type":"string","enum":["handoff_requested","handoff_acknowledged","quiesced","lost"]},
                "observed_at":{"type":"string","format":"date-time"},
                "evidence":{"type":"string","minLength":1,"maxLength":2048}
              }}
          }}))
}

pub(super) fn inspect_tool() -> Value {
    tool_def(INSPECT_CEREMONY_RESUME_TOOL,
        "Read a bounded pause/drain and resume preflight. Separates paused admission from external host declarations; lists live/expired/retired claims, owners, original fences, operation identity, absolute deadlines, receipt evidence and advisory recovery paths. No clock changes, host-liveness inference or takeover permission. Follow next_after_claim until empty.",
        json!({"type":"object","additionalProperties":false,"required":["ceremony_id"],
          "properties":{"ceremony_id":string_schema("Ceremony to inspect."),
            "after_claim":string_schema("Last claim fence from the previous page."),
            "limit":{"type":"integer","minimum":1,"maximum":1000,"default":100}}}))
}
