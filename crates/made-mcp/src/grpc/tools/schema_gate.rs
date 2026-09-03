//! Every field a request builder demands must be declared required
//! in the schema that tool publishes.
//!
//! Twice now a tool has required a field its published schema said
//! nothing about: `role_kind` on the intervention verbs, and
//! `actor_id` on `made_start_ceremony`, whose declaration landed
//! on an identically-shaped block one function away. Both shipped
//! green, because nothing compared the two sides.
//!
//! The check is behavioural rather than structural: build the
//! arguments a caller reading the schema would send — exactly the
//! declared required fields, nothing else — and hand them to the
//! builder. A builder that answers "missing required string `x`"
//! is asking for something the schema never told anyone to send.

use serde_json::{json, Map, Value};

use crate::protocol::tools_list_result;

/// Tools whose builder takes no arguments worth declaring.
///
/// Listed rather than skipped by accident: a new tool has to be
/// put on one side of this line or the other.
const NO_ARGUMENTS: [&str; 4] = [
    "made_list_councils",
    "made_list_contracts",
    "made_get_status",
    "made_get_metrics",
];

/// What a caller reading this schema would send, and no more.
///
/// A `oneOf` or `anyOf` of alternative requirements is satisfied by
/// its first branch. Picking one is the point: a schema that states
/// its alternatives can be driven, and one that leaves them to
/// prose cannot.
fn minimal_arguments(schema: &Value) -> Value {
    let mut required = schema
        .get("required")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for key in ["oneOf", "anyOf"] {
        if let Some(branch) = schema
            .get(key)
            .and_then(Value::as_array)
            .and_then(|branches| branches.first())
            .and_then(|branch| branch.get("required"))
            .and_then(Value::as_array)
        {
            required.extend(branch.iter().cloned());
        }
    }
    let properties = schema.get("properties").and_then(Value::as_object);
    let mut arguments = Map::new();
    for field in required {
        let Some(name) = field.as_str() else { continue };
        let property = properties.and_then(|properties| properties.get(name));
        arguments.insert(name.to_owned(), value_for(property));
    }
    Value::Object(arguments)
}

/// A plausible value of the declared type.
///
/// Plausible, not valid: the builders check presence and shape, and
/// domain validation happens further in. A value that gets past the
/// builder is all this needs.
fn value_for(property: Option<&Value>) -> Value {
    let Some(property) = property else {
        return json!("x");
    };
    if let Some(first) = property
        .get("enum")
        .and_then(Value::as_array)
        .and_then(|variants| variants.first())
    {
        return first.clone();
    }
    match property.get("type").and_then(Value::as_str) {
        Some("object") => minimal_arguments(property),
        Some("array") => json!([]),
        Some("integer" | "number") => json!(1),
        Some("boolean") => json!(true),
        _ => json!("x"),
    }
}

#[test]
fn no_builder_demands_a_field_its_schema_never_declared() {
    let catalog = tools_list_result(|_| true);
    let tools = catalog["tools"].as_array().expect("a tool catalog");
    assert!(!tools.is_empty(), "the catalog served no tools");

    let mut unchecked = Vec::new();
    for tool in tools {
        let name = tool["name"].as_str().expect("every tool is named");
        if NO_ARGUMENTS.contains(&name) {
            continue;
        }
        let arguments = minimal_arguments(&tool["inputSchema"]);
        let Some(outcome) = built(name, &arguments) else {
            unchecked.push(name.to_owned());
            continue;
        };
        if let Err(complaint) = outcome {
            assert!(
                !complaint.contains("missing required"),
                "{name} demands something its schema does not declare: {complaint}\n\
                 sent exactly what the schema asks for: {arguments}"
            );
        }
    }

    // Not a failure, but not silence either: a tool whose builder
    // this gate cannot drive is a tool the gate is not covering,
    // and saying so beats reading a green test as full coverage.
    if !unchecked.is_empty() {
        eprintln!("schema gate could not drive: {unchecked:?}");
    }
}

/// The request a builder makes of these arguments, rendered, or
/// what it complained about instead.
///
/// Rendered rather than typed because every builder returns a
/// different message and this gate holds them all the same way.
fn built(name: &str, arguments: &Value) -> Option<Result<String, String>> {
    let outcome = match name {
        "made_deliberate" => rendered(build_deliberate_request(arguments)),
        "made_stream_deliberation" => rendered(build_stream_deliberation_request(arguments)),
        "made_get_deliberation_result" => {
            rendered(build_get_deliberation_result_request(arguments))
        }
        "made_orchestrate" => rendered(build_orchestrate_request(arguments)),
        "made_create_council" => rendered(build_create_council_request(arguments)),
        "made_delete_council" => rendered(build_delete_council_request(arguments)),
        "made_register_agent" => rendered(build_register_agent_request(arguments)),
        "made_unregister_agent" => rendered(build_unregister_agent_request(arguments)),
        "made_process_trigger_event" => rendered(build_process_trigger_event_request(arguments)),
        "made_run_council_decision" => rendered(build_run_council_decision_request(arguments)),
        "made_register_contract" => rendered(build_register_contract_request(arguments)),
        "made_delete_contract" => rendered(build_delete_contract_request(arguments)),
        "made_run_ceremony" => rendered(build_run_ceremony_request(arguments)),
        "made_start_ceremony" => rendered(build_start_ceremony_request(arguments)),
        "made_start_published_ceremony" => {
            rendered(build_start_published_ceremony_request(arguments))
        }
        "made_run_ceremony_step" => rendered(build_run_ceremony_step_request(arguments)),
        "made_apply_ceremony_transition" => {
            rendered(build_apply_ceremony_transition_request(arguments))
        }
        "made_approve_ceremony_guard" => rendered(build_approve_ceremony_guard_request(arguments)),
        "made_defer_ceremony_guard" => rendered(build_defer_ceremony_guard_request(arguments)),
        "made_request_ceremony_intervention" => {
            rendered(build_request_ceremony_intervention_request(arguments))
        }
        "made_respond_to_ceremony_intervention" => {
            rendered(build_respond_to_ceremony_intervention_request(arguments))
        }
        "made_close_ceremony_intervention" => {
            rendered(build_close_ceremony_intervention_request(arguments))
        }
        "made_collect_ceremony_evidence" => {
            rendered(build_collect_ceremony_evidence_request(arguments))
        }
        "made_assert_ceremony_reason" => rendered(build_assert_ceremony_reason_request(arguments)),
        _ => return None,
    };
    Some(outcome)
}

fn rendered<T: std::fmt::Debug>(outcome: Result<T, String>) -> Result<String, String> {
    outcome.map(|request| format!("{request:?}"))
}

/// A field the schema calls required must reach the request.
///
/// The other way this pair can disagree, and the one that is worse:
/// a builder that fills a declared field with a value of its own
/// never complains about anything. `made_run_ceremony` shipped
/// like that — every call sent `actor_id: "operator-1"` whatever the
/// caller declared, which is the engine writing down an actor
/// nobody chose, the one thing the field exists to prevent.
///
/// Told apart by building twice with different values: a builder
/// that reads the field produces two different requests, and one
/// that invents produces the same one twice.
#[test]
fn every_required_field_reaches_the_request() {
    let catalog = tools_list_result(|_| true);
    for tool in catalog["tools"].as_array().expect("a tool catalog") {
        let name = tool["name"].as_str().expect("every tool is named");
        if NO_ARGUMENTS.contains(&name) {
            continue;
        }
        let schema = &tool["inputSchema"];
        let baseline = minimal_arguments(schema);
        let Some(fields) = baseline.as_object().map(|fields| {
            fields
                .iter()
                .filter(|(_, value)| value.is_string())
                .map(|(field, _)| field.clone())
                .collect::<Vec<_>>()
        }) else {
            continue;
        };

        for field in fields {
            // Enums have a fixed vocabulary, so "another value" is
            // not something this gate can invent for them.
            if schema["properties"][&field].get("enum").is_some() {
                continue;
            }
            let mut other = baseline.clone();
            other[&field] = json!("y");
            let (Some(Ok(first)), Some(Ok(second))) = (built(name, &baseline), built(name, &other))
            else {
                continue;
            };
            assert_ne!(
                first, second,
                "{name} builds the same request whether `{field}` says one thing or another: \
                 it is not reading the field it declares required"
            );
        }
    }
}

use super::{
    build_apply_ceremony_transition_request, build_approve_ceremony_guard_request,
    build_assert_ceremony_reason_request, build_close_ceremony_intervention_request,
    build_collect_ceremony_evidence_request, build_create_council_request,
    build_defer_ceremony_guard_request, build_delete_contract_request,
    build_delete_council_request, build_deliberate_request, build_get_deliberation_result_request,
    build_orchestrate_request, build_process_trigger_event_request, build_register_agent_request,
    build_register_contract_request, build_request_ceremony_intervention_request,
    build_respond_to_ceremony_intervention_request, build_run_ceremony_request,
    build_run_ceremony_step_request, build_run_council_decision_request,
    build_start_ceremony_request, build_start_published_ceremony_request,
    build_stream_deliberation_request, build_unregister_agent_request,
};
