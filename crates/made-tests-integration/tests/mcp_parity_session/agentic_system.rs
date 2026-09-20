//! One system, designed, sealed and run on both engines.
//!
//! The pin carries no digest: the engine resolves it from what is
//! published, and both engines publish the same ceremony, so the two
//! answers must name the same bytes. That is the whole claim of this
//! family — the same document produces the same digest wherever it is
//! decoded — and it is checked here rather than asserted in prose.

use serde_json::{json, Value};

/// The design, and the run of it.
pub(super) const SYSTEM_ID: &str = "parity-system";
const EXECUTION_ID: &str = "parity-system-run";

pub(super) fn script() -> Vec<(&'static str, Value)> {
    vec![
        ("made_design_agentic_system", json!({ "design": design() })),
        ("made_get_agentic_system", json!({ "system_id": SYSTEM_ID })),
        ("made_list_agentic_systems", json!({ "limit": 10 })),
        (
            "made_validate_agentic_system",
            json!({ "system_id": SYSTEM_ID, "revision": 1 }),
        ),
        (
            "made_publish_agentic_system",
            json!({ "system_id": SYSTEM_ID, "revision": 1 }),
        ),
        (
            "made_instantiate_agentic_system",
            json!({
                "system_id": SYSTEM_ID,
                "revision": 1,
                "execution_id": EXECUTION_ID,
                "inputs": { "delivery": {} },
                "offers": {
                    "delivery_agent": { "specialty": "triage", "capabilities": ["drafting"] }
                },
                "actor_id": "parity-operator",
                "actor_kind": "service",
            }),
        ),
        (
            "made_get_agentic_system_execution",
            json!({ "execution_id": EXECUTION_ID }),
        ),
        (
            "made_advance_agentic_system_execution",
            json!({
                "execution_id": EXECUTION_ID,
                "actor_id": "parity-operator",
                "actor_kind": "service",
            }),
        ),
        (
            "made_render_agentic_system_diagram",
            json!({ "system_id": SYSTEM_ID, "execution_id": EXECUTION_ID }),
        ),
    ]
}

/// A system with one composition, seated exactly as the published
/// definition declares.
///
/// Minimal on purpose: what this family proves is that two engines
/// agree, and a larger design would only make a disagreement harder
/// to locate.
fn design() -> Value {
    json!({
        "id": SYSTEM_ID,
        "purpose": "prove that two engines read one design the same way",
        "integrator_role_id": "integrator",
        "roles": [
            {
                "id": "integrator",
                "responsibility": "drives the system from outside it",
                "kind": "integrator",
            },
            {
                "id": "deliverer",
                "responsibility": "does the declared work",
                "kind": "contributor",
            },
        ],
        "participants": [
            {
                "id": "operator",
                "role": "integrator",
                "kind": "person",
            },
            {
                "id": "delivery_agent",
                "role": "deliverer",
                "kind": "agent",
                "binding": { "capabilities": ["drafting"] },
            },
        ],
        "topology": [
            {
                "from": "operator",
                "to": "delivery_agent",
                "kind": "coordination",
                "handoff": true,
            }
        ],
        "profiles": {
            "deliverer": {
                "requested_model": "balanced-model",
                "requested_reasoning_effort": "medium",
                "required_capabilities": ["drafting"],
                "fallback_policy": "fallback",
                "fallback_model": "small-model",
            }
        },
        "ceremonies": [
            {
                "id": "delivery",
                "pin": { "name": "parity_published", "version": "1.0" },
                "purpose": "carry out the delivery",
                "activation": { "kind": "manual" },
                "role_bindings": { "FACILITATOR": "delivery_agent" },
            }
        ],
    })
}
