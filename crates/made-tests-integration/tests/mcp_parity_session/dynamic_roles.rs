//! Dynamic claims resolve from current context on both MCP backends.

use super::*;

use super::optionals::checked;

const CEREMONY_ID: &str = "parity-dynamic-role-repeat";

fn design_intent() -> Value {
    json!({
        "name": "parity_dynamic_role_repeat",
        "objective": "Reassign the reviewer between complete review rounds.",
        "outputs": ["review"],
        "participants": [
            {"role_id": "A"},
            {"role_id": "B"},
            {"role_id": "C"}
        ],
        "max_parallel": 2,
        "stages": [{
            "id": "review",
            "group": {
                "execution": "concurrent",
                "steps": [
                    {
                        "id": "dynamic_review",
                        "owner_role_id": "B",
                        "instructions": "Review as the role currently selected in context.",
                        "handler": "host_callback",
                        "role_from": "context.next_role",
                        "allowed_roles": ["B", "C"]
                    },
                    {
                        "id": "choose_next",
                        "owner_role_id": "A",
                        "instructions": "Choose the reviewer for the following work.",
                        "handler": "host_callback",
                        "context_writes": {"next_role": "assigned"}
                    }
                ],
                "join": {"condition": "all_steps_completed"},
                "repeat": {
                    "max_iterations": 2,
                    "until": {
                        "step": "dynamic_review",
                        "output_field": "approved",
                        "equals": true
                    }
                }
            }
        }]
    })
}

#[tokio::test]
async fn dynamic_claim_tracks_context_across_state_iterations_in_memory() {
    drive_dynamic_role_session(&ParityArms::start().await).await;
}

#[tokio::test]
async fn dynamic_claim_tracks_context_across_state_iterations_on_sqlite() {
    drive_dynamic_role_session(&ParityArms::start_on_the_shipped_store().await).await;
}

async fn drive_dynamic_role_session(arms: &ParityArms) {
    let designed = checked(arms, 4_000, "made_design_ceremony", design_intent()).await;
    let yaml = structured(&designed)["definition_yaml"]
        .as_str()
        .expect("the designer returns YAML");
    assert!(yaml.contains("role_from: context.next_role"), "{yaml}");
    assert!(yaml.contains("allowed_roles:"), "{yaml}");
    assert!(yaml.contains("context_writes:"), "{yaml}");

    let started = checked(
        arms,
        4_001,
        "made_start_ceremony",
        json!({
            "ceremony_id": CEREMONY_ID,
            "definition_yaml": yaml,
            "context": {"next_role": "B"},
            "actor_id": "parity-operator",
            "actor_kind": "service"
        }),
    )
    .await;
    assert_eq!(structured(&started)["context"]["next_role"], "B");

    claim_dynamic(arms, 4_002, 1).await;
    claim_writer(arms, 4_003, 1).await;
    complete_writer(arms, 4_004, "C").await;
    let first_completion = complete_dynamic(arms, 4_005, false).await;
    assert_eq!(structured(&first_completion)["current_state_iteration"], 2);
    assert_eq!(structured(&first_completion)["context"]["next_role"], "C");

    claim_dynamic(arms, 4_006, 2).await;
    claim_writer(arms, 4_007, 2).await;
    complete_writer(arms, 4_008, "B").await;
    let second_completion = complete_dynamic(arms, 4_009, true).await;
    assert_eq!(structured(&second_completion)["current_state_iteration"], 2);
    assert_eq!(structured(&second_completion)["context"]["next_role"], "B");

    checked(
        arms,
        4_010,
        "made_apply_ceremony_transition",
        json!({
            "ceremony_id": CEREMONY_ID,
            "trigger": "review_completed",
            "actor_kind": "agent"
        }),
    )
    .await;

    let history = checked(
        arms,
        4_011,
        "made_read_ceremony_events",
        json!({"ceremony_id": CEREMONY_ID}),
    )
    .await;
    assert_dynamic_audit(structured(&history));
}

async fn claim_dynamic(arms: &ParityArms, id: u64, state_iteration: u32) {
    let answer = checked(
        arms,
        id,
        "made_claim_ceremony_step",
        json!({
            "ceremony_id": CEREMONY_ID,
            "step_id": "dynamic_review",
            "actor_kind": "agent",
            "lease_owner_id": "parity-dynamic-host",
            "idempotency_key": format!("dynamic-{state_iteration}"),
            "lease_ttl_ms": 60_000
        }),
    )
    .await;
    let record = step(structured(&answer), "dynamic_review");
    assert_eq!(record["status"], "in_progress");
    assert_eq!(record["state_iteration"], state_iteration);
}

async fn claim_writer(arms: &ParityArms, id: u64, state_iteration: u32) {
    checked(
        arms,
        id,
        "made_claim_ceremony_step",
        json!({
            "ceremony_id": CEREMONY_ID,
            "step_id": "choose_next",
            "actor_kind": "agent",
            "lease_owner_id": "parity-dynamic-host",
            "idempotency_key": format!("writer-{state_iteration}"),
            "lease_ttl_ms": 60_000
        }),
    )
    .await;
}

async fn complete_writer(arms: &ParityArms, id: u64, assigned: &str) {
    checked(
        arms,
        id,
        "made_complete_ceremony_step",
        json!({
            "ceremony_id": CEREMONY_ID,
            "step_id": "choose_next",
            "actor_kind": "agent",
            "status": "completed",
            "output": {"assigned": assigned}
        }),
    )
    .await;
}

async fn complete_dynamic(arms: &ParityArms, id: u64, approved: bool) -> Value {
    checked(
        arms,
        id,
        "made_complete_ceremony_step",
        json!({
            "ceremony_id": CEREMONY_ID,
            "step_id": "dynamic_review",
            "actor_kind": "agent",
            "status": "completed",
            "output": {"approved": approved}
        }),
    )
    .await
}

fn step<'a>(instance: &'a Value, step_id: &str) -> &'a Value {
    instance["steps"]
        .as_array()
        .expect("an instance carries its steps")
        .iter()
        .find(|record| record["step_id"] == step_id)
        .unwrap_or_else(|| panic!("instance has no `{step_id}` step: {instance:#}"))
}

fn assert_dynamic_audit(history: &Value) {
    let records = history["records"]
        .as_array()
        .expect("history carries audit records");
    let dynamic_starts = records
        .iter()
        .filter(|record| {
            record["event_type"] == "step_started" && record["event"]["step_id"] == "dynamic_review"
        })
        .collect::<Vec<_>>();
    assert_eq!(dynamic_starts.len(), 2, "{history:#}");
    for (record, role, state_iteration) in
        [(dynamic_starts[0], "B", 1), (dynamic_starts[1], "C", 2)]
    {
        assert_eq!(record["event"]["started_by"], role);
        assert_eq!(record["actor"]["role_id"], role);
        assert_eq!(record["event"]["state_iteration"], state_iteration);
    }

    let dynamic_completions = records
        .iter()
        .filter(|record| {
            record["event_type"] == "step_completed"
                && record["event"]["step_id"] == "dynamic_review"
        })
        .collect::<Vec<_>>();
    assert_eq!(dynamic_completions.len(), 2, "{history:#}");
    for (record, role, state_iteration) in [
        (dynamic_completions[0], "B", 1),
        (dynamic_completions[1], "C", 2),
    ] {
        assert_eq!(record["event"]["finished_by"], role);
        assert_eq!(record["actor"]["role_id"], role);
        assert_eq!(record["event"]["state_iteration"], state_iteration);
    }
}
