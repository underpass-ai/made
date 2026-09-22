use serde_json::{json, Value};

/// A session of this part's own, so the loop's bindings and offers do
/// not disturb the definitions other parts publish and diff.
const LOOP_SESSION_ID: &str = "parity-integrator-loop-session";

/// The binding both arms take, and the key the scripted host files each
/// arm's own ticket under.
const BINDING_ID: &str = "parity-loop-binding";

const LOOP_CEREMONY: &str = r#"
version: "1.0"
name: "parity_integrator_loop"
states:
  - id: OPEN
    initial: true
  - id: DONE
    terminal: true
transitions:
  - from: OPEN
    to: DONE
    trigger: finish
steps:
  - id: work
    state: OPEN
    handler: noop
roles:
  - id: ENGINEER
    allowed_actions:
      - work
      - finish
  - id: INTEGRATOR
    allowed_actions:
      - finish
"#;

/// The integrator loop, driven on both backends.
///
/// Nothing here enqueues a delivery: a binding is taken, a real step is
/// claimed and sealed, and the item that comes back is whatever the
/// append woke. A scripted queue would prove the presenters agree and
/// nothing about whether either engine is connected.
///
/// The lease is minted per engine, so the acknowledgements present the
/// ticket their own arm was issued — comparing one scripted literal
/// against two different tickets would compare an acknowledgement with
/// a refusal and call the difference parity.
pub(super) fn script() -> Vec<(&'static str, Value)> {
    let mut calls = take_the_scope();
    calls.extend(one_result_taken_up());
    calls
}

/// Open a session and put one host in charge of it.
fn take_the_scope() -> Vec<(&'static str, Value)> {
    vec![
        (
            "made_start_ceremony",
            json!({
                "ceremony_id": LOOP_SESSION_ID,
                "definition_yaml": LOOP_CEREMONY,
                "actor_id": "parity-operator",
                "actor_kind": "service",
            }),
        ),
        (
            "made_bind_ceremony_integrator",
            json!({
                "binding_id": BINDING_ID,
                "scope": { "kind": "ceremony", "ceremony_id": LOOP_SESSION_ID },
                "role_id": "INTEGRATOR",
                "host_kind": "claude-code",
                "address": "parity-loop-session",
                // Stated rather than left to the defaults, so the two
                // engines are compared on how they read terms a caller
                // chose as well as on the terms they invent.
                "activation": "none",
                "incarnation": "parity-loop-incarnation-1",
                "replace": false,
                "follow_replacement": false,
            }),
        ),
        (
            "made_get_ceremony_integrator_binding",
            json!({ "scope": { "kind": "ceremony", "ceremony_id": LOOP_SESSION_ID } }),
        ),
        // The other half of the scope fork, which authorizes globally
        // rather than against a ceremony (ADR-021). Nobody is driving
        // this run, and both engines have to say so the same way
        // through three different guards: the rpc macro, the embedded
        // authorizer and the facade.
        (
            "made_get_ceremony_integrator_binding",
            json!({ "scope": { "kind": "system_execution", "system_execution_id": "parity-loop-unbound-run" } }),
        ),
        // Nothing has happened yet, so this is the empty answer a host
        // has to be able to read: no items, and a loop state that says
        // whether coming back is worth it.
        //
        // The wait is stated rather than left to the default, and it is
        // stated as a number a caller could have meant: a zero optional
        // means "take the engine's own" on this contract, so scripting
        // zero would compare the two engines' defaults rather than what
        // they do with an instruction.
        (
            "made_await_integrator_attention",
            json!({
                "scope": { "kind": "ceremony", "ceremony_id": LOOP_SESSION_ID },
                "binding_id": BINDING_ID,
                "incarnation": "parity-loop-incarnation-1",
                "fence": 0,
                "wait_timeout_ms": 50,
            }),
        ),
    ]
}

/// Real work, sealed through the ordinary commands, and what the loop
/// did with it.
fn one_result_taken_up() -> Vec<(&'static str, Value)> {
    vec![
        // Real work, sealed through the ordinary commands. The claim
        // fence is filled in by the scripted host, per arm.
        (
            "made_claim_ceremony_step",
            json!({
                "ceremony_id": LOOP_SESSION_ID,
                "step_id": "work",
                "actor_kind": "agent",
                "lease_owner_id": "parity-loop-host",
                "idempotency_key": "parity-loop-claim",
                "lease_ttl_ms": 60_000,
            }),
        ),
        (
            "made_complete_ceremony_step",
            json!({
                "ceremony_id": LOOP_SESSION_ID,
                "step_id": "work",
                "actor_kind": "agent",
                "status": "completed",
                "output": { "finding": "the queue drained", "attachments": 1 },
            }),
        ),
        // The append woke the projection; this is what it offered.
        (
            "made_await_integrator_attention",
            json!({
                "scope": { "kind": "ceremony", "ceremony_id": LOOP_SESSION_ID },
                "binding_id": BINDING_ID,
                "incarnation": "parity-loop-incarnation-1",
                "fence": 0,
                "limit": 10,
                "wait_timeout_ms": 50,
                "lease_duration_ms": 60_000,
            }),
        ),
        // Intent while the lease is held, then the effect. Two calls,
        // in that order, because that is the sequence discovery
        // documents and the one a crash has to be readable through.
        (
            "made_acknowledge_integrator_attention",
            json!({
                "binding_id": BINDING_ID,
                "incarnation": "parity-loop-incarnation-1",
                "fence": 0,
                "delivery_id": format!("$pulled:{BINDING_ID}"),
                "lease_id": "$pulled",
                "acknowledgement": "intent",
                "action_kind": "integrated",
                "idempotency_key": "parity-loop-integration-1",
                "note": "the integrator is taking the result up",
            }),
        ),
        (
            "made_acknowledge_integrator_attention",
            json!({
                "binding_id": BINDING_ID,
                "incarnation": "parity-loop-incarnation-1",
                "fence": 0,
                "delivery_id": format!("$pulled:{BINDING_ID}"),
                "lease_id": "$pulled",
                "acknowledgement": "processed",
                "action_kind": "integrated",
                "idempotency_key": "parity-loop-integration-1",
            }),
        ),
        (
            "made_list_attention_deliveries",
            json!({ "binding_id": BINDING_ID, "limit": 10 }),
        ),
        (
            "made_list_attention_deliveries",
            json!({ "ceremony_id": LOOP_SESSION_ID, "state": "processed" }),
        ),
    ]
}
