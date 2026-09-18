//! A state-repeat session crosses both MCP backends and both shipped stores.

use super::*;

use super::optionals::checked;

const CEREMONY_ID: &str = "parity-state-repeat";
const CONSUMER: &str = "parity-state-repeat-consumer";

fn design_intent() -> Value {
    json!({
        "name": "parity_state_repeat",
        "objective": "Reach agreement after a complete concurrent review round.",
        "outputs": ["review"],
        "participants": [{"role_id": "A"}, {"role_id": "B"}],
        "max_parallel": 2,
        "stages": [{
            "id": "review",
            "group": {
                "execution": "concurrent",
                "steps": [
                    {
                        "id": "a",
                        "owner_role_id": "A",
                        "instructions": "Review the first perspective.",
                        "handler": "host_callback"
                    },
                    {
                        "id": "b",
                        "owner_role_id": "B",
                        "instructions": "Approve only after both perspectives agree.",
                        "handler": "host_callback"
                    }
                ],
                "join": {"condition": "all_steps_completed"},
                "repeat": {
                    "max_iterations": 2,
                    "until": {"step": "b", "output_field": "approved", "equals": true}
                }
            }
        }]
    })
}

#[tokio::test]
async fn concurrent_state_repeat_has_full_mcp_session_parity_in_memory() {
    drive_state_repeat_session(&ParityArms::start().await).await;
}

#[tokio::test]
async fn concurrent_state_repeat_has_full_mcp_session_parity_on_sqlite() {
    drive_state_repeat_session(&ParityArms::start_on_the_shipped_store().await).await;
}

async fn drive_state_repeat_session(arms: &ParityArms) {
    let designed = checked(arms, 3_000, "made_design_ceremony", design_intent()).await;
    let yaml = structured(&designed)["definition_yaml"]
        .as_str()
        .expect("the designer returns YAML");
    assert!(yaml.contains("execution: concurrent"), "{yaml}");
    assert!(yaml.contains("max_iterations: 2"), "{yaml}");
    assert!(yaml.contains("step: b"), "{yaml}");
    assert!(yaml.contains("output_field: approved"), "{yaml}");

    let started = checked(
        arms,
        3_001,
        "made_start_ceremony",
        json!({
            "ceremony_id": CEREMONY_ID,
            "definition_yaml": yaml,
            "actor_id": "parity-operator",
            "actor_kind": "service"
        }),
    )
    .await;
    assert_iteration(structured(&started), 1, &["a", "b"]);

    claim(arms, 3_002, "a", 1).await;
    claim(arms, 3_003, "b", 1).await;

    let b_first = complete(arms, 3_004, "b", 1, false).await;
    let instance = structured(&b_first);
    assert_eq!(instance["current_state_iteration"], 1);
    assert_eq!(step(instance, "b")["status"], "completed");
    assert_eq!(step(instance, "b")["state_iteration"], 1);
    assert_eq!(step(instance, "a")["status"], "in_progress");
    assert_eq!(instance["state_repeat_condition_satisfied"], false);
    let before_boundary = history(arms, 3_005).await;
    assert_eq!(boundary_count(&before_boundary), 0);

    let a_last = complete(arms, 3_006, "a", 1, false).await;
    assert_iteration(structured(&a_last), 2, &["a", "b"]);
    assert_eq!(
        structured(&a_last)["state_repeat_condition_satisfied"],
        false
    );
    let after_boundary = history(arms, 3_007).await;
    assert_boundary_follows_last_completion(&after_boundary);

    claim(arms, 3_008, "a", 2).await;
    claim(arms, 3_009, "b", 2).await;
    let a_second = complete(arms, 3_010, "a", 2, true).await;
    assert_eq!(structured(&a_second)["current_state_iteration"], 2);
    assert_eq!(step(structured(&a_second), "b")["status"], "in_progress");

    let b_second = complete(arms, 3_011, "b", 2, true).await;
    let instance = structured(&b_second);
    assert_eq!(instance["current_state_iteration"], 2);
    assert_eq!(instance["state_repeat_condition_satisfied"], true);
    assert_eq!(instance["state_repeat_limit_reached"], false);
    assert_eq!(instance["transitions"][0]["trigger"], "review_completed");
    assert_eq!(instance["transitions"][0]["enabled"], true);

    let finished = checked(
        arms,
        3_012,
        "made_apply_ceremony_transition",
        json!({
            "ceremony_id": CEREMONY_ID,
            "trigger": "review_completed",
            "actor_kind": "agent"
        }),
    )
    .await;
    assert_eq!(structured(&finished)["completed"], true);

    assert_durable_views(arms).await;
}

async fn claim(arms: &ParityArms, id: u64, step_id: &str, state_iteration: u32) {
    let answer = checked(
        arms,
        id,
        "made_claim_ceremony_step",
        json!({
            "ceremony_id": CEREMONY_ID,
            "step_id": step_id,
            "actor_kind": "agent",
            "lease_owner_id": "parity-state-repeat-host",
            "idempotency_key": format!("state-{state_iteration}-{step_id}"),
            "lease_ttl_ms": 60_000
        }),
    )
    .await;
    let record = step(structured(&answer), step_id);
    assert_eq!(record["status"], "in_progress");
    assert_eq!(record["state_iteration"], state_iteration);
}

async fn complete(
    arms: &ParityArms,
    id: u64,
    step_id: &str,
    state_iteration: u32,
    approved: bool,
) -> Value {
    let answer = checked(
        arms,
        id,
        "made_complete_ceremony_step",
        json!({
            "ceremony_id": CEREMONY_ID,
            "step_id": step_id,
            "actor_kind": "agent",
            "status": "completed",
            "output": {"approved": approved, "state_iteration": state_iteration}
        }),
    )
    .await;
    answer
}

async fn history(arms: &ParityArms, id: u64) -> Value {
    checked(
        arms,
        id,
        "made_read_ceremony_events",
        json!({"ceremony_id": CEREMONY_ID}),
    )
    .await
}

fn step<'a>(instance: &'a Value, step_id: &str) -> &'a Value {
    instance["steps"]
        .as_array()
        .expect("an instance carries its steps")
        .iter()
        .find(|step| step["step_id"] == step_id)
        .unwrap_or_else(|| panic!("instance has no `{step_id}` step: {instance:#}"))
}

fn assert_iteration(instance: &Value, state_iteration: u32, claimable: &[&str]) {
    assert_eq!(instance["current_state_iteration"], state_iteration);
    assert_eq!(instance["state_repeat_max_iterations"], 2);
    assert_eq!(instance["claimable_step_ids"], json!(claimable));
    for step_id in ["a", "b"] {
        let record = step(instance, step_id);
        assert_eq!(record["state_iteration"], state_iteration);
        assert_eq!(record["status"], "pending");
    }
}

fn records(history: &Value) -> &[Value] {
    structured(history)["records"]
        .as_array()
        .expect("history carries records")
}

fn boundary_count(history: &Value) -> usize {
    records(history)
        .iter()
        .filter(|record| record["event_type"] == "state_iteration_started")
        .count()
}

fn assert_boundary_follows_last_completion(history: &Value) {
    let records = records(history);
    let boundary = records
        .iter()
        .position(|record| record["event_type"] == "state_iteration_started")
        .expect("the first complete round opens a state boundary");
    assert_eq!(boundary_count(history), 1);
    let completed = &records[boundary - 1];
    assert_eq!(completed["event_type"], "step_completed");
    assert_eq!(completed["event"]["step_id"], "a");
    assert_eq!(completed["event"]["state_iteration"], 1);
    assert_eq!(records[boundary]["event"]["state_iteration"], 2);
}

async fn assert_durable_views(arms: &ParityArms) {
    let history = history(arms, 3_013).await;
    assert_boundary_follows_last_completion(&history);
    let completion_coordinates = records(&history)
        .iter()
        .filter(|record| record["event_type"] == "step_completed")
        .map(|record| {
            (
                record["event"]["step_id"].as_str().unwrap(),
                record["event"]["state_iteration"].as_u64().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        completion_coordinates,
        [("b", 1), ("a", 1), ("a", 2), ("b", 2)]
    );

    let transcript = checked(
        arms,
        3_014,
        "made_get_ceremony_transcript",
        json!({"ceremony_id": CEREMONY_ID}),
    )
    .await;
    let transcript = structured(&transcript);
    assert_eq!(transcript["entry_count"], 4);
    assert_eq!(
        transcript["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| (
                entry["step_id"].as_str().unwrap(),
                entry["state_iteration"].as_u64().unwrap(),
            ))
            .collect::<Vec<_>>(),
        [("b", 1), ("a", 1), ("a", 2), ("b", 2)]
    );

    let pulled = checked(
        arms,
        3_015,
        "made_pull_ceremony_events",
        json!({"consumer": CONSUMER, "limit": 100}),
    )
    .await;
    let pulled_records = structured(&pulled)["records"].as_array().unwrap();
    assert_eq!(pulled_records.len(), records(&history).len());
    for (pulled, historical) in pulled_records.iter().zip(records(&history)) {
        let mut pulled = pulled.clone();
        pulled
            .as_object_mut()
            .expect("a pulled record is an object")
            .remove("global_position");
        assert_eq!(&pulled, historical);
    }
    let last_position = pulled_records
        .last()
        .and_then(|record| record["global_position"].as_u64())
        .expect("the feed positions every record");
    let acknowledged = checked(
        arms,
        3_016,
        "made_pull_ceremony_events",
        json!({
            "consumer": CONSUMER,
            "limit": 100,
            "acknowledge_through": last_position
        }),
    )
    .await;
    assert_eq!(structured(&acknowledged)["records"], json!([]));
    assert_eq!(
        structured(&acknowledged)["acknowledged_through"],
        last_position
    );

    let verdict = checked(
        arms,
        3_017,
        "made_verify_ceremony_journal",
        json!({"ceremony_id": CEREMONY_ID}),
    )
    .await;
    assert_eq!(structured(&verdict)["intact"], true);
    assert_eq!(
        structured(&verdict)["record_count"],
        records(&history).len()
    );
}
