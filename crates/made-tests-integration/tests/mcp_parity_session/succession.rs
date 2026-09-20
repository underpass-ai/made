//! Handing the paused budget session to a successor, on both backends.
//!
//! The predecessor arrives here paused with one outstanding claim, put
//! there by the host-handoff script. What this adds is the handoff
//! itself: read the plan, seal it and open the successor, retry the
//! same request, retry it with different content, and try to resume
//! the ceremony that has been superseded.

use super::{assert_same_answer, failed, ParityArms, BUDGET_SESSION_ID};
use serde_json::{json, Value};

/// The successor's definition: the same ceremony at a version that
/// still declares `work`, so the diff carries rather than strands it.
pub(super) const SUCCESSOR_CEREMONY: &str = r#"
version: "2.0"
name: "parity_published"
states:
  - id: OPEN
    initial: true
  - id: REVIEWED
  - id: DONE
    terminal: true
transitions:
  - from: OPEN
    to: REVIEWED
    trigger: finish
  - from: REVIEWED
    to: DONE
    trigger: accept
steps:
  - id: work
    state: OPEN
    handler: parity_step
roles:
  - id: FACILITATOR
    allowed_actions:
      - work
      - finish
      - accept
"#;

fn handoff(dispositions: &Value) -> Value {
    json!({
        "ceremony_id": BUDGET_SESSION_ID,
        "plan_id": "parity-handoff-1",
        "definition_name": "parity_published",
        "definition_version": "2.0",
        "carried": [],
        "dispositions": dispositions,
        "budget": "fresh",
        "actor_id": "parity-operator",
        "actor_kind": "service",
    })
}

pub(super) fn script() -> Vec<(&'static str, Value)> {
    vec![
        (
            "made_publish_ceremony_definition",
            json!({ "definition_yaml": SUCCESSOR_CEREMONY }),
        ),
        (
            "made_plan_ceremony_successor",
            json!({
                "ceremony_id": BUDGET_SESSION_ID,
                "definition_name": "parity_published",
                "definition_version": "2.0",
            }),
        ),
        (
            "made_start_ceremony_successor",
            handoff(&json!([{ "step_id": "work", "kind": "retry_in_successor" }])),
        ),
        // The same request again: the handoff is already sealed and the
        // successor's stream already opened, so this verifies both
        // instead of making either a second time.
        (
            "made_start_ceremony_successor",
            handoff(&json!([{ "step_id": "work", "kind": "retry_in_successor" }])),
        ),
        // Read back both directions of the relation.
        (
            "made_get_ceremony_instance",
            json!({ "ceremony_id": BUDGET_SESSION_ID }),
        ),
    ]
}

/// The two refusals, which the scripted run cannot carry because every
/// call in it has to succeed.
///
/// Both backends have to refuse for the same reason and in the same
/// words: a refusal that differed between them would be a difference in
/// what the engine is, discovered by whoever happened to be on the
/// other transport.
pub(super) async fn assert_both_refuse_a_superseded_origin(arms: &ParityArms) {
    // The same plan id asking for something else is a different handoff
    // wearing one name.
    let conflicting = arms.completing(
        "made_start_ceremony_successor",
        handoff(&json!([{ "step_id": "work", "kind": "abandon_no_external_effect" }])),
    );
    let (wire, embedded) = arms
        .call(9_001, "made_start_ceremony_successor", &conflicting)
        .await;
    assert!(
        failed(&wire),
        "a different handoff under one plan id conflicts: {wire:#}"
    );
    assert!(
        failed(&embedded),
        "a different handoff under one plan id conflicts: {embedded:#}"
    );
    assert_same_answer("made_start_ceremony_successor", &wire, &embedded);

    // A ceremony that named its successor has said which definition it
    // believes in, and it is not this one.
    let resume = json!({
        "ceremony_id": BUDGET_SESSION_ID,
        "actor_id": "parity-operator",
        "actor_kind": "service",
    });
    let (wire, embedded) = arms.call(9_002, "made_resume_ceremony", &resume).await;
    assert!(
        failed(&wire),
        "a superseded ceremony does not resume: {wire:#}"
    );
    assert!(
        failed(&embedded),
        "a superseded ceremony does not resume: {embedded:#}"
    );
    assert_same_answer("made_resume_ceremony", &wire, &embedded);
}
