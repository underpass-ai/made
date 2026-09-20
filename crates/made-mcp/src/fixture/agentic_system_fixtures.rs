//! Canned answers for the agentic-system tools.
//!
//! Shaped like the real ones so a client written against the fixture
//! backend finds the same keys in the same places when it is pointed
//! at a real engine. A fixture whose shape differed would be a demo
//! that teaches the wrong contract.

use serde_json::{json, Value};

const SYSTEM_ID: &str = "integrator-delivery";
const DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// Which tools this fixture answers.
///
/// One arm per tool below rather than a combined pattern: the catalog
/// gate scans this source for each tool's own arm, and a tool folded
/// into a neighbour's pattern would read as one nothing answers.
pub(super) fn handles(name: &str) -> bool {
    matches!(
        name,
        "made_design_agentic_system"
            | "made_get_agentic_system"
            | "made_list_agentic_systems"
            | "made_validate_agentic_system"
            | "made_publish_agentic_system"
            | "made_instantiate_agentic_system"
            | "made_advance_agentic_system_execution"
            | "made_get_agentic_system_execution"
            | "made_render_agentic_system_diagram"
    )
}

pub(super) fn response(name: &str) -> Value {
    match name {
        "made_design_agentic_system" => system(),
        "made_get_agentic_system" => system(),
        "made_list_agentic_systems" => json!({
            "systems": [{
                "system_id": SYSTEM_ID,
                "revision": 1,
                "lifecycle": "draft",
                "digest": DIGEST,
                "purpose": "deliver what was asked for, and show that it was reviewed",
            }],
            "next_cursor": null,
        }),
        "made_validate_agentic_system" => json!({
            "system_id": SYSTEM_ID,
            "revision": 1,
            "publishable": true,
            "error_count": 0,
            "warning_count": 0,
            "findings": [],
            "resolved_pins": [{
                "ceremony": "delivery",
                "name": "integrator_delivery",
                "version": "1.0",
                "digest": DIGEST,
            }],
        }),
        "made_publish_agentic_system" => json!({
            "outcome": "published",
            "system_id": SYSTEM_ID,
            "sealed_revision": 1,
            "head_revision": 2,
            "digest": DIGEST,
        }),
        "made_instantiate_agentic_system" => execution(),
        "made_advance_agentic_system_execution" => execution(),
        "made_get_agentic_system_execution" => {
            let mut rendered = execution();
            if let Some(object) = rendered.as_object_mut() {
                object.insert("system".to_owned(), system());
            }
            rendered
        }
        "made_render_agentic_system_diagram" => json!({
            "mermaid": "flowchart LR\n  subgraph lane_users [\"Users\"]\n    users[\"whoever asked for this\"]\n  end",
            "text_equivalent": [
                "System `integrator-delivery` at revision 1: deliver what was asked for, and show that it was reviewed",
            ],
        }),
        _ => unreachable!("only agentic system tools reach this fixture"),
    }
}

fn system() -> Value {
    json!({
        "system_id": SYSTEM_ID,
        "revision": 1,
        "lifecycle": "draft",
        "digest": DIGEST,
        "yaml": "id: integrator-delivery\nrevision: 1\nlifecycle: draft\n",
        "system": {"id": SYSTEM_ID, "revision": 1, "lifecycle": "draft"},
    })
}

fn execution() -> Value {
    json!({
        "execution_id": "run-1",
        "system_id": SYSTEM_ID,
        "revision": 1,
        "digest": DIGEST,
        "state": "running",
        "integrator_binding_id": null,
        "participants": [{
            "participant": "operator",
            "bound": true,
            "specialty": "operator",
            "unavailable_because": null,
        }],
        "ceremonies": [{
            "ceremony": "delivery",
            "pin": {"name": "integrator_delivery", "version": "1.0", "digest": DIGEST},
            "planned": "started",
            "round": 1,
            "instance_id": "run-1-delivery-1",
            "skipped_because": null,
        }],
    })
}
