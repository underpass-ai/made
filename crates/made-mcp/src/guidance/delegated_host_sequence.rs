use std::collections::BTreeSet;

use serde_json::{json, Value};

use crate::protocol::{
    APPLY_CEREMONY_TRANSITION_TOOL, CLAIM_CEREMONY_STEP_TOOL, COMPLETE_CEREMONY_STEP_TOOL,
    GET_CEREMONY_INSTANCE_TOOL, REPORT_CEREMONY_AGENT_STATUS_TOOL,
};

pub(super) fn delegated_host_sequence(names: &BTreeSet<String>) -> Vec<Value> {
    let required = [
        CLAIM_CEREMONY_STEP_TOOL,
        COMPLETE_CEREMONY_STEP_TOOL,
        GET_CEREMONY_INSTANCE_TOOL,
        APPLY_CEREMONY_TRANSITION_TOOL,
    ];
    if !required.iter().all(|tool| names.contains(*tool)) {
        return Vec::new();
    }

    vec![
        json!({
            "order": 1,
            "tool": CLAIM_CEREMONY_STEP_TOOL,
            "instruction": "Claim the exact next_step_id with stable lease owner and idempotency key. This acquires a lease; it performs no stage work."
        }),
        json!({
            "order": 2,
            "host_action": true,
            "instruction": "Resolve and retain the explicit host execution profile (requested versus actual model/effort, capabilities, fallback, host agent/incarnation and inheritance). Perform the stage's real work through that authorized worker and tools. A checkpoint or handoff records provenance for a later claim; it does not change a running agent's model."
        }),
        json!({
            "order": 3,
            "tool": COMPLETE_CEREMONY_STEP_TOOL,
            "instruction": "Only after real work finishes, record its observable status and structured output with evidence/artifact references. Never file attempted or simulated work as completed."
        }),
        json!({
            "order": 4,
            "tool": REPORT_CEREMONY_AGENT_STATUS_TOOL,
            "instruction": "Report bounded activity/checkpoint evidence with logical worker, host agent/incarnation, claim fence, sequence and idempotency key; never include chain-of-thought or secrets."
        }),
        json!({
            "order": 5,
            "tool": GET_CEREMONY_INSTANCE_TOOL,
            "instruction": "Refresh the instance and verify the persisted step status and output."
        }),
        json!({
            "order": 6,
            "tool": APPLY_CEREMONY_TRANSITION_TOOL,
            "instruction": "Apply only a transition reported as enabled; pause for unresolved guards or interventions."
        }),
    ]
}
