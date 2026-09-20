//! The one canned session every ceremony move answers with.
//!
//! Its own file because it is the largest single shape the fixture
//! backend holds, and because a client reading it is reading what a
//! ceremony looks like rather than how the fixture is wired.

use serde_json::{json, Value};

/// A working session with something in every collection, so a client
/// wiring against the fixture meets the same shape a live engine
/// answers — including the parts that only appear once a person has
/// taken part.
pub(super) fn ceremony_instance_fixture() -> Value {
    json!({
        "ceremony_id": "ceremony-fixture-1",
        "trace_id": null,
        "correlation_id": null,
        "causation_id": null,
        "definition_name": "fixture_ceremony",
        "definition_version": "1.0",
        "bound_definition_digest": null,
        "current_state": "REVIEW",
        "current_state_iteration": 0,
        "current_state_visit": 0,
        "state_repeat_max_iterations": null,
        "state_repeat_condition_satisfied": false,
        "state_repeat_limit_reached": false,
        "completed": false,
        "next_step_id": null,
        "claimable_step_ids": [],
        "waiting_for_human": ["human_approved"],
        "guard_deferrals": [
            {
                "guard_name": "budget_approved",
                "statement": "Not approving today.",
                "reason": "The cost figure is a guess.",
                "reconsider_when": ["a measured figure exists"],
                "deferred_at": "2026-01-01T00:00:00Z"
            }
        ],
        "transitions": [
            {
                "trigger": "approve",
                "to_state": "DONE",
                "enabled": false,
                "guards": [
                    { "name": "human_approved", "kind": "human", "satisfied": false }
                ]
            }
        ],
        "steps": [
            {
                "step_id": "collect_context",
                "state_id": "OPEN",
                "status": "completed",
                "attempt": 1,
                "output": { "summary": "the team agreed on the brief" },
                "error": null
            }
        ],
        "interventions": [
            {
                "intervention_id": "item-1",
                "kind": "investigation",
                "status": "open",
                "requested_by": "FACILITATOR",
                "target": { "kind": "table" },
                "request": {
                    "message": "Which rollback did we rehearse last?",
                    "details": {}
                },
                "provenance": null,
                "responses": [
                    {
                        "role_id": "RISK_REVIEWER",
                        "message": "The one from the March release.",
                        "details": {},
                        "evidence_pack": null,
                        "responded_at": "2026-01-01T00:00:00Z"
                    }
                ],
                "created_at": "2026-01-01T00:00:00Z",
                "updated_at": "2026-01-01T00:00:00Z",
                "closed_at": null
            }
        ],
        "open_intervention_ids": ["item-1"],
        "participant_bindings": [
            {
                "role_id": "RISK_REVIEWER",
                "specialty": "senior_sre_panel",
                "bound_at": "2026-01-01T00:00:00Z"
            }
        ],
        "context": {
            "brief": "ship the editorial calendar",
            "memory_scope": "team:editorial"
        },
        "recollection": fixture_recollection(),
        "lineage": null,
        "child_groups": []
    })
}

fn fixture_recollection() -> Value {
    json!({
        "scope": "team:editorial",
        "truncated": false,
        "entries": [{
            "entry_id": "guard:budget_approved",
            "kind": "decision",
            "summary": "`budget_approved` was approved",
            "from_ceremony_id": "ceremony-fixture-0",
            "observed_at": "2025-12-01T00:00:00Z"
        }]
    })
}
