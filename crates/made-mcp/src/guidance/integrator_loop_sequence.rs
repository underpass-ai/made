//! The order an integrator drives a whole scope in.
//!
//! Written down rather than left to be inferred, because the two
//! places a host goes wrong are both invisible from any single call:
//! acting on a batch without reading the ceremony again, and never
//! stopping. Both are steps here.

use std::collections::BTreeSet;

use serde_json::{json, Value};

use crate::protocol::{
    ACKNOWLEDGE_INTEGRATOR_ATTENTION_TOOL, AWAIT_INTEGRATOR_ATTENTION_TOOL,
    BIND_CEREMONY_INTEGRATOR_TOOL, GET_CEREMONY_INSTANCE_TOOL,
    GET_CEREMONY_INTEGRATOR_BINDING_TOOL, LIST_ATTENTION_DELIVERIES_TOOL,
};

/// The loop states a host stops on rather than asking again.
pub(super) const HALTING_STATES: [&str; 4] =
    ["completed", "failed", "blocked", "awaiting_human_decision"];

pub(super) fn integrator_loop_sequence(names: &BTreeSet<String>) -> Vec<Value> {
    let required = [
        BIND_CEREMONY_INTEGRATOR_TOOL,
        AWAIT_INTEGRATOR_ATTENTION_TOOL,
        ACKNOWLEDGE_INTEGRATOR_ATTENTION_TOOL,
        GET_CEREMONY_INSTANCE_TOOL,
    ];
    if !required.iter().all(|tool| names.contains(*tool)) {
        return Vec::new();
    }

    vec![
        json!({
            "order": 1,
            "tool": BIND_CEREMONY_INTEGRATOR_TOOL,
            "instruction": format!(
                "Bind this host to the scope, naming your own incarnation. Keep the binding id and the fence the answer carries: every later call is checked against them. Read {GET_CEREMONY_INTEGRATOR_BINDING_TOOL} first if you do not know whether somebody else already holds the scope; a live binding is refused rather than displaced unless you ask for it."
            )
        }),
        json!({
            "order": 2,
            "tool": AWAIT_INTEGRATOR_ATTENTION_TOOL,
            "instruction": "Ask for what you are owed, with the binding id, the incarnation and the fence. The wait is bounded and capped at 30000ms; an empty batch with end_reason wait_elapsed means ask again."
        }),
        json!({
            "order": 3,
            "tool": GET_CEREMONY_INSTANCE_TOOL,
            "instruction": "Read the ceremony again before acting. What travelled with the batch was true when the batch was built; the item names what happened, and the instance says what is true now."
        }),
        json!({
            "order": 4,
            "tool": ACKNOWLEDGE_INTEGRATOR_ATTENTION_TOOL,
            "instruction": "Record the intent while you still hold the lease: acknowledgement=intent, naming the act you are about to perform and the idempotency key you will perform it under."
        }),
        json!({
            "order": 5,
            "host_action": true,
            "instruction": "Do the work through the ordinary authorized commands, under that same idempotency key. The loop hands out items and records what was said about them; it performs nothing and authorizes nothing."
        }),
        json!({
            "order": 6,
            "tool": ACKNOWLEDGE_INTEGRATOR_ATTENTION_TOOL,
            "instruction": "Only after the effect lands, acknowledgement=processed with the same act. Use acknowledgement=failed with a reason when it did not: that counts an attempt and may offer the item again, which is how at-least-once delivery stays honest."
        }),
        json!({
            "order": 7,
            "tool": AWAIT_INTEGRATOR_ATTENTION_TOOL,
            "instruction": format!(
                "Ask again, and keep going while the loop is running. Stop when loop_state is one of {}, and hand the scope back to a person. Items are delivered at least once: dedupe on delivery_id, because an item you have already processed can be offered again after a crash. {LIST_ATTENTION_DELIVERIES_TOOL} is the paperwork an operator reads when it is unclear what was offered to whom.",
                HALTING_STATES.join(", ")
            )
        }),
    ]
}
